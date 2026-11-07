use std::collections::{HashMap, HashSet};

use crate::net::buf::{ReadError, Reader};
use crate::net::scene::Scene;
use crate::world::{CellUpdate, Circle, Kind, Player, Rgb, Sym};

const NAME_COLOR: Rgb = Rgb(255, 255, 255);

fn team_color(Rgb(r, g, b): Rgb) -> Rgb {
    if r >= g && r >= b {
        Rgb(255, 0, 0)
    } else if g >= b {
        Rgb(0, 255, 0)
    } else {
        Rgb(0, 0, 255)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Node {
    pub(crate) kind: Kind,
    pub(crate) at: Circle,
    pub(crate) color: Rgb,
    pub(crate) name: Sym,
    pub(crate) skin: Sym,
    pub(crate) owner: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Cells {
    pub(crate) nodes: HashMap<u32, Node>,
    pub(crate) second_skin: Sym,
    own: [HashSet<u32>; 2],
}

impl Cells {
    pub(crate) fn alive(&self) -> bool {
        self.own.iter().any(|o| !o.is_empty())
    }

    pub(crate) fn unit_of(&self, raw: u32) -> Option<u32> {
        self.own
            .iter()
            .position(|o| o.contains(&raw))
            .map(|unit| unit as u32)
    }

    pub(crate) fn claim(&mut self, unit: usize, raw: u32) {
        self.own[1 - unit].remove(&raw);
        self.own[unit].insert(raw);
    }

    pub(crate) fn release_own(&mut self) {
        self.own.iter_mut().for_each(HashSet::clear);
    }

    pub(crate) fn reset(&mut self) {
        self.nodes.clear();
        self.own.iter_mut().for_each(HashSet::clear);
    }

    pub(crate) fn read_removals(
        &mut self,
        r: &mut Reader,
        eaten: &[(u32, u32)],
        scene: &mut Scene,
    ) -> Result<(), ReadError> {
        for &(prey, hunter) in eaten {
            self.remove(prey, Some(hunter), scene);
        }
        for _ in 0..r.u16()? {
            let raw = r.u32()?;
            self.remove(raw, None, scene);
        }
        Ok(())
    }

    fn remove(&mut self, raw: u32, eater: Option<u32>, scene: &mut Scene) {
        if self.nodes.remove(&raw).is_none() {
            return;
        }
        for own in &mut self.own {
            own.remove(&raw);
        }
        scene.remove(raw, eater);
    }

    pub(crate) fn write(&self, id: u32, node: &Node, teams: bool, scene: &mut Scene) {
        let own = self.unit_of(id).is_some();
        if own {
            scene.add_own(id);
        }
        scene.upsert(CellUpdate {
            id,
            kind: node.kind,
            at: node.at,
            color: node.color,
            owner: node.owner,
        });
        if node.kind == Kind::Player {
            scene.set_player(
                node.owner,
                Player {
                    name: node.name,
                    skins: [
                        node.skin,
                        if own && self.second_skin != Sym::NONE {
                            self.second_skin
                        } else {
                            node.skin
                        },
                    ],
                    team_color: if teams {
                        team_color(node.color)
                    } else {
                        NAME_COLOR
                    },
                },
            );
        }
    }
}
