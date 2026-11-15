//! The compact protocol: protocol 6 with smaller world updates. The server
//! speaks it when the handshake key is [`HANDSHAKE_KEY`] and says so with a
//! hello frame first; protocol 6 servers that do not know it ignore the key, so
//! the codec decodes whichever arrives. See `PROTOCOL.md` in the server.

use std::collections::HashSet;

use super::{skin_url, Ogar, SELF_IDS};
use crate::proto::buf::{ReadError, Reader};
use crate::proto::cells::Node;
use crate::proto::map::publish_map;
use crate::proto::Cx;
use crate::world::scene::Scene;
use crate::world::{Circle, Kind, Rgb, ServerView, Sym};

/// "MOG1" read as a little-endian u32.
pub(super) const HANDSHAKE_KEY: u32 = u32::from_le_bytes(*b"MOG1");

pub(super) const OP_HELLO: u8 = 0x50;
pub(super) const OP_PLAYER: u8 = 0x51;
pub(super) const OP_WORLD: u8 = 0x52;

const KIND_FOOD: u8 = 0;
const KIND_EJECTED: u8 = 1;
const KIND_VIRUS: u8 = 2;
const KIND_MOTHER: u8 = 3;
const KIND_PLAYER: u8 = 4;
const KIND_OWN: u8 = 5;

const CHANGED_X: u8 = 0x01;
const CHANGED_Y: u8 = 0x02;
const CHANGED_RADIUS: u8 = 0x04;
const CHANGED_COLOR: u8 = 0x08;

/// What the server last told of a cell, in its integer wire units; updates are
/// differences from it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Shown {
    x: i32,
    y: i32,
    radius: u32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Told {
    name: Sym,
    skin: Sym,
}

/// Ids in a section are sent as differences from the previous one.
struct Ids(u32);

impl Ids {
    fn next(&mut self, delta: u64) -> u32 {
        self.0 = self.0.wrapping_add(delta as u32);
        self.0
    }
}

fn rgb(r: &mut Reader) -> Result<Rgb, ReadError> {
    Ok(Rgb(r.u8()?, r.u8()?, r.u8()?))
}

fn kind_of(code: u8) -> Kind {
    match code {
        KIND_FOOD => Kind::Food,
        KIND_EJECTED => Kind::Eject,
        KIND_VIRUS => Kind::Virus,
        KIND_MOTHER => Kind::Mother,
        _ => Kind::Player,
    }
}

impl Ogar {
    pub(super) fn read_hello(&mut self, r: &mut Reader) -> Result<(), ReadError> {
        let _version = r.u8()?;
        self.compact = true;
        Ok(())
    }

    pub(super) fn read_player(&mut self, r: &mut Reader, scene: &mut Scene) -> Result<(), ReadError> {
        let pid = r.varint()? as u32;
        let name = scene.intern(r.utf8z()?);
        let skin = scene.intern(&skin_url(r.utf8z()?));
        self.told.insert(pid, Told { name, skin });
        Ok(())
    }

    fn at(&self, s: Shown) -> Circle {
        Circle {
            x: s.x as f32 - self.offset.0,
            y: s.y as f32 - self.offset.1,
            r: s.radius as f32,
        }
    }

    /// Mirrors protocol 6, where the camera (0x11) comes before UpdateNodes and
    /// only counts while the client has no cells.
    pub(super) fn read_world(
        &mut self,
        r: &mut Reader,
        cx: &mut Cx,
        scene: &mut Scene,
    ) -> Result<(), ReadError> {
        let camera = (r.signed()? as i32, r.signed()? as i32);
        if !self.cells.alive() {
            scene.set_server_view(Some(ServerView {
                x: camera.0 as f32 - self.offset.0,
                y: camera.1 as f32 - self.offset.1,
            }));
        }

        self.awaiting_border = false;
        scene.begin(cx.now);
        let second = skin_url(&cx.identity.skin2);
        self.cells.second_skin = if second.is_empty() {
            Sym::NONE
        } else {
            scene.intern(&second)
        };
        let tab = cx.tab.index();
        let teams = self.teams();

        let mut eaten = Vec::new();
        let mut ids = Ids(0);
        for _ in 0..r.varint()? {
            let prey = ids.next(r.varint()?);
            let hunter = r.varint()? as u32;
            eaten.push((prey, hunter));
        }

        // Protocol 6 resends every player cell in view each tick, and the scene
        // takes a repeat at the same place as having stopped; those the compact
        // protocol leaves out are repeated below.
        let mut touched = HashSet::new();
        let mut ids = Ids(0);
        for _ in 0..r.varint()? {
            let head = r.varint()?;
            let raw = ids.next(head >> 4);
            touched.insert(raw);
            let changed = head as u8 & 0x0f;
            let mut shown = self.shown.get(&raw).copied().unwrap_or_default();
            if changed & CHANGED_X != 0 {
                shown.x = shown.x.wrapping_add(r.signed()? as i32);
            }
            if changed & CHANGED_Y != 0 {
                shown.y = shown.y.wrapping_add(r.signed()? as i32);
            }
            if changed & CHANGED_RADIUS != 0 {
                shown.radius = (i64::from(shown.radius) + r.signed()?) as u32;
            }
            let color = if changed & CHANGED_COLOR != 0 {
                Some(rgb(r)?)
            } else {
                None
            };
            let Some(known) = self.cells.nodes.get(&raw).copied() else {
                continue;
            };
            self.shown.insert(raw, shown);
            let mut node = Node {
                at: self.at(shown),
                color: color.unwrap_or(known.color),
                ..known
            };
            node.owner = self.owner_for(raw, &node, tab, scene);
            self.cells.nodes.insert(raw, node);
            self.cells.write(raw, &node, teams, scene);
        }

        let mut ids = Ids(0);
        for _ in 0..r.varint()? {
            let head = r.varint()?;
            let raw = ids.next(head >> 3);
            touched.insert(raw);
            let code = head as u8 & 0x07;
            let shown = Shown {
                x: camera.0.wrapping_add(r.signed()? as i32),
                y: camera.1.wrapping_add(r.signed()? as i32),
                radius: r.varint()? as u32,
            };
            let color = rgb(r)?;
            let told = if code >= KIND_PLAYER {
                self.told.get(&(r.varint()? as u32)).copied()
            } else {
                None
            };
            if code >= KIND_OWN {
                self.cells.claim(usize::from(code - KIND_OWN) & 1, raw);
            }
            self.shown.insert(raw, shown);
            let mut node = Node {
                kind: kind_of(code),
                at: self.at(shown),
                color,
                name: told.map_or(Sym::NONE, |t| t.name),
                skin: told.map_or(Sym::NONE, |t| t.skin),
                owner: 0,
            };
            node.owner = self.owner_for(raw, &node, tab, scene);
            self.cells.nodes.insert(raw, node);
            self.cells.write(raw, &node, teams, scene);
        }

        let mut still: Vec<u32> = self
            .cells
            .nodes
            .iter()
            .filter(|(raw, node)| node.kind == Kind::Player && !touched.contains(raw))
            .map(|(&raw, _)| raw)
            .collect();
        still.sort_unstable();
        for raw in still {
            let mut node = self.cells.nodes[&raw];
            node.owner = self.owner_for(raw, &node, tab, scene);
            self.cells.nodes.insert(raw, node);
            self.cells.write(raw, &node, teams, scene);
        }

        let mut ids = Ids(0);
        let mut removed = Vec::new();
        for _ in 0..r.varint()? {
            removed.push(ids.next(r.varint()?));
        }
        for &(prey, hunter) in &eaten {
            self.shown.remove(&prey);
            self.cells.remove(prey, Some(hunter), scene);
        }
        for &raw in &removed {
            self.shown.remove(&raw);
            self.cells.remove(raw, None, scene);
        }

        let controlled = self
            .cells
            .alive()
            .then(|| SELF_IDS[tab] + self.active_unit as u32);
        if controlled.is_some() {
            scene.set_server_view(None);
        }
        scene.set_controlled_unit(controlled);
        if cx.publishes {
            publish_map(cx, scene.world());
        }
        Ok(())
    }
}
