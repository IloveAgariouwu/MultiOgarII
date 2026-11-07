use serde::Deserialize;

use super::cells::{Cells, Node};
use super::map::publish_map;
use super::pacer::{Pacer, FEED_INTERVAL_MS};
use super::{Act, Cx, RecordPolicy};
use crate::net::buf::{ReadError, Reader, Writer};
use crate::net::hud::{hex_color, ChatLine, LeaderRow, NetEvent, Population, TeamRow};
use crate::net::scene::{PeerKey, Scene};
use crate::skin_url::is_imgur_skin;
use crate::world::{Border, Circle, Kind, Rgb, ServerView, Sym};

const PROTOCOL_VERSION: u32 = 6;
const HANDSHAKE_KEY: u32 = 1;
const CHAT_MAX_BYTES: usize = 253;
const LOAD_PER_TICK_MS: f64 = 2.5;
pub(super) const SELF_IDS: [u32; 2] = [2, 4];
const TEAMS_GAME_TYPE: u32 = 1;
const TEAM_COLORS: [&str; 3] = ["#df0000", "#00df00", "#0000df"];
const UNNAMED_CELL: &str = "An unnamed cell";
const SKIN_BASE: &str = "./skins/";
const SMALLEST_PLAYER: u16 = 32;
const MOTHER_COLOR: Rgb = Rgb(206, 99, 99);

mod op {
    pub const JOIN: u8 = 0;
    pub const SPECTATE: u8 = 1;
    pub const MOUSE: u8 = 16;
    pub const SPLIT: u8 = 17;
    pub const FREE_ROAM: u8 = 18;
    pub const FEED: u8 = 21;
    /// Xingkong's Tab: the server then sends owned cells with their unit
    /// (0x20 unit 0, 0x21 unit 1) and the active unit (0x22).
    pub const SWITCH_UNIT: u8 = 44;
    pub const CHAT: u8 = 99;
    pub const PROTOCOL: u8 = 254;
    pub const STAT: u8 = 254;
    pub const HANDSHAKE: u8 = 255;
}

mod flag {
    pub const VIRUS: u8 = 0x01;
    pub const COLOR: u8 = 0x02;
    pub const SKIN: u8 = 0x04;
    pub const NAME: u8 = 0x08;
    pub const AGITATED: u8 = 0x10;
    pub const EJECT: u8 = 0x20;
}

#[derive(Debug, Default, Clone)]
pub struct Ogar {
    cells: Cells,
    offset: (f32, f32),
    awaiting_border: bool,
    game_type: u32,
    active_unit: usize,
    free_roam: bool,
    pacer: Pacer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerStat {
    name: String,
    mode: String,
    uptime: u64,
    update: String,
    players_total: u32,
    players_limit: u32,
    players_alive: u32,
    players_spect: u32,
}

fn uptime(seconds: u64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    if days > 0 {
        format!("{days} d")
    } else if hours > 0 {
        format!("{hours} h")
    } else {
        format!("{minutes} min")
    }
}

fn display_name(wire: &str) -> &str {
    let name = wire
        .strip_prefix('<')
        .and_then(|body| {
            let head = body.find('}').map_or(body, |end| &body[..end]);
            head.rfind('>').map(|close| &body[close + 1..])
        })
        .unwrap_or(wire);
    name.trim()
}

fn skin_url(wire: &str) -> String {
    let name = wire.strip_prefix('%').unwrap_or(wire).trim();
    if is_imgur_skin(name) {
        return name.to_owned();
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    if name.is_empty() || name.starts_with('.') || !name.chars().all(safe) {
        return String::new();
    }
    format!("{SKIN_BASE}{name}.png")
}

fn packet(build: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::default();
    build(&mut w);
    w.finish()
}

impl Ogar {
    fn kind_of(flags: u8, size: u16, color: Rgb) -> Kind {
        if flags & flag::EJECT != 0 {
            Kind::Eject
        } else if flags & (flag::VIRUS | flag::AGITATED) != 0 {
            if color == MOTHER_COLOR {
                Kind::Mother
            } else {
                Kind::Virus
            }
        } else if flags & (flag::NAME | flag::SKIN) == 0 && size < SMALLEST_PLAYER {
            Kind::Food
        } else {
            Kind::Player
        }
    }

    fn teams(&self) -> bool {
        self.game_type == TEAMS_GAME_TYPE
    }

    fn owner_for(&self, raw: u32, node: &Node, tab: usize, scene: &mut Scene) -> u32 {
        if node.kind != Kind::Player {
            return 0;
        }
        if let Some(unit) = self.cells.unit_of(raw) {
            return SELF_IDS[tab] + unit;
        }
        scene.peer(PeerKey {
            name: node.name,
            skin: node.skin,
            color: [node.color.0, node.color.1, node.color.2],
        })
    }

    fn read_nodes(
        &mut self,
        r: &mut Reader,
        cx: &mut Cx,
        scene: &mut Scene,
    ) -> Result<(), ReadError> {
        self.awaiting_border = false;
        scene.begin(cx.now);
        let second = skin_url(&cx.identity.skin2);
        self.cells.second_skin = if second.is_empty() {
            Sym::NONE
        } else {
            scene.intern(&second)
        };
        let mut eaten = Vec::new();
        for _ in 0..r.u16()? {
            let hunter = r.u32()?;
            let prey = r.u32()?;
            eaten.push((prey, hunter));
        }
        let tab = cx.tab.index();
        let teams = self.teams();
        loop {
            let raw = r.u32()?;
            if raw == 0 {
                break;
            }
            let x = r.i32()? as f32 - self.offset.0;
            let y = r.i32()? as f32 - self.offset.1;
            let size = r.u16()?;
            let flags = r.u8()?;
            let color = if flags & flag::COLOR != 0 {
                Some(Rgb(r.u8()?, r.u8()?, r.u8()?))
            } else {
                None
            };
            let skin = if flags & flag::SKIN != 0 {
                Some(scene.intern(&skin_url(r.utf8z()?)))
            } else {
                None
            };
            let name = if flags & flag::NAME != 0 {
                Some(scene.intern(r.utf8z()?))
            } else {
                None
            };
            let at = Circle {
                x,
                y,
                r: f32::from(size),
            };
            let mut node = match self.cells.nodes.get(&raw) {
                Some(known) => Node {
                    at,
                    color: color.unwrap_or(known.color),
                    name: name.unwrap_or(known.name),
                    skin: skin.unwrap_or(known.skin),
                    ..*known
                },
                None => Node {
                    kind: Self::kind_of(flags, size, color.unwrap_or_default()),
                    at,
                    color: color.unwrap_or_default(),
                    name: name.unwrap_or(Sym::NONE),
                    skin: skin.unwrap_or(Sym::NONE),
                    owner: 0,
                },
            };
            node.owner = self.owner_for(raw, &node, tab, scene);
            self.cells.nodes.insert(raw, node);
            self.cells.write(raw, &node, teams, scene);
        }
        self.cells.read_removals(r, &eaten, scene)?;
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

    fn read_border(&mut self, r: &mut Reader, scene: &mut Scene) -> Result<(), ReadError> {
        let min_x = r.f64()? as f32;
        let min_y = r.f64()? as f32;
        let max_x = r.f64()? as f32;
        let max_y = r.f64()? as f32;
        let long = r.remaining() >= 4;
        if long {
            self.game_type = r.u32()?;
        }
        let center = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        let close =
            (center.0 - self.offset.0).abs() <= 1.0 && (center.1 - self.offset.1).abs() <= 1.0;
        if !(long || self.awaiting_border || close) {
            return Ok(());
        }
        self.offset = center;
        self.awaiting_border = false;
        let half = ((max_x - min_x) * 0.5, (max_y - min_y) * 0.5);
        scene.set_border(Border {
            min_x: -half.0,
            min_y: -half.1,
            max_x: half.0,
            max_y: half.1,
        });
        Ok(())
    }

    fn read_leaderboard(r: &mut Reader, cx: &mut Cx) -> Result<(), ReadError> {
        let count = r.u32()?;
        let mut rows = Vec::new();
        for i in 0..count {
            let is_self = r.u32()? != 0;
            let name = match display_name(r.utf8z()?) {
                "" => UNNAMED_CELL.to_owned(),
                name => name.to_owned(),
            };
            rows.push(LeaderRow {
                rank: i + 1,
                name,
                is_self,
            });
        }
        if cx.publishes {
            cx.hud.leaderboard = Some(rows);
        }
        Ok(())
    }

    fn read_teams(r: &mut Reader, cx: &mut Cx) -> Result<(), ReadError> {
        let count = r.u32()?;
        let mut rows = Vec::new();
        for i in 0..count {
            let share = f64::from(r.f32()?);
            rows.push(TeamRow {
                color: TEAM_COLORS[i as usize % TEAM_COLORS.len()].to_owned(),
                share,
            });
        }
        if cx.publishes {
            cx.hud.teams = Some(rows);
        }
        Ok(())
    }

    fn read_chat(r: &mut Reader, cx: &mut Cx) -> Result<(), ReadError> {
        let flags = r.u8()?;
        let color = hex_color(r.u8()?, r.u8()?, r.u8()?);
        let name = display_name(r.utf8z()?).to_owned();
        let text = r.utf8z()?.to_owned();
        if cx.publishes {
            cx.hud.chat.push(ChatLine {
                name,
                text,
                color,
                system: flags & 0x80 != 0,
            });
        }
        Ok(())
    }

    fn read_stat(&mut self, r: &mut Reader, cx: &mut Cx) -> Result<(), ReadError> {
        let stat: ServerStat =
            serde_json::from_str(r.utf8z()?).map_err(|e| ReadError::Json(e.to_string()))?;
        let tick_ms: f64 = stat
            .update
            .parse()
            .map_err(|_| ReadError::Json(format!("update `{}` is not a number", stat.update)))?;
        cx.hud.population = Some(Population {
            total: stat.players_total,
            max: stat.players_limit,
            players: stat.players_alive,
            spectators: stat.players_spect,
        });
        cx.hud.server_status = Some(format!(
            "{} ({})\nLoad: {:.1} %\nUptime: {}",
            stat.name,
            stat.mode,
            tick_ms * LOAD_PER_TICK_MS,
            uptime(stat.uptime)
        ));
        cx.latency = self.pacer.pong(cx.now);
        Ok(())
    }

    fn clear(&mut self, scene: &mut Scene) {
        scene.clear();
        self.cells.reset();
        self.active_unit = 0;
        self.awaiting_border = true;
    }

    pub(crate) fn is_following_largest(&self) -> bool {
        !self.free_roam
    }

    pub(crate) fn on_open(&mut self, cx: &mut Cx) {
        self.awaiting_border = true;
        cx.send(packet(|w| {
            w.u8(op::PROTOCOL).u32(PROTOCOL_VERSION);
        }));
        cx.send(packet(|w| {
            w.u8(op::HANDSHAKE).u32(HANDSHAKE_KEY);
        }));
        if cx.spectating {
            self.send_spectate(cx);
        }
    }

    pub(crate) fn receive(
        &mut self,
        data: &[u8],
        cx: &mut Cx,
        scene: &mut Scene,
    ) -> Result<(), ReadError> {
        let mut r = Reader::new(data);
        match r.u8()? {
            0x10 => self.read_nodes(&mut r, cx, scene)?,
            0x11 => {
                let x = r.f32()? - self.offset.0;
                let y = r.f32()? - self.offset.1;
                r.skip(4)?;
                if !self.cells.alive() {
                    scene.set_server_view(Some(ServerView { x, y }));
                }
            }
            0x12 => self.clear(scene),
            0x14 => self.cells.release_own(),
            0x20 => self.cells.claim(0, r.u32()?),
            0x21 => self.cells.claim(1, r.u32()?),
            0x22 => self.active_unit = usize::from(r.u8()? & 1),
            0x31 => Self::read_leaderboard(&mut r, cx)?,
            0x32 => Self::read_teams(&mut r, cx)?,
            0x40 => self.read_border(&mut r, scene)?,
            0x63 => Self::read_chat(&mut r, cx)?,
            0xfe => self.read_stat(&mut r, cx)?,
            op => return Err(ReadError::Opcode(op)),
        }
        Ok(())
    }

    pub(crate) fn aim(&mut self, x: f32, y: f32, cx: &mut Cx) {
        let (ox, oy) = self.offset;
        cx.send(packet(|w| {
            w.u8(op::MOUSE)
                .i32((x + ox).round() as i32)
                .i32((y + oy).round() as i32)
                .u32(0);
        }));
    }

    pub(crate) fn send_join(&mut self, cx: &mut Cx) {
        let id = cx.identity;
        let join = if id.skin.is_empty() {
            id.name.clone()
        } else {
            format!("<{}>{}", id.skin, id.name)
        };
        cx.send(packet(|w| {
            w.u8(op::JOIN).utf8z(&join);
        }));
    }

    pub(crate) fn send_spectate(&mut self, cx: &mut Cx) {
        cx.send(vec![op::SPECTATE]);
    }

    pub(crate) fn set_free_roam(&mut self, free: bool, cx: &mut Cx) {
        if self.cells.alive() {
            return;
        }
        if free != self.free_roam {
            self.free_roam = free;
            cx.send(vec![op::FREE_ROAM]);
        }
    }

    pub(crate) fn send_action(&mut self, act: Act, cx: &mut Cx) {
        match act {
            Act::Split(1) => cx.send(vec![op::SPLIT]),
            Act::Split(n) => self.pacer.queue_splits(cx.now, n),
            Act::Feed(down) => {
                if self.pacer.feed(down, cx.now, FEED_INTERVAL_MS) {
                    cx.send(vec![op::FEED]);
                }
            }
            Act::FeedOne => cx.send(vec![op::FEED]),
            Act::ChangeUnit => cx.send(vec![op::SWITCH_UNIT]),
        }
    }

    pub(crate) fn release_control(&mut self) {
        self.pacer.stop_feed();
    }

    pub(crate) fn send_chat(&mut self, text: &str, cx: &mut Cx) {
        if text.len() > CHAT_MAX_BYTES {
            cx.hud.events.push(NetEvent::ChatTooLong);
            return;
        }
        cx.send(packet(|w| {
            w.u8(op::CHAT).u8(0).utf8z(text);
        }));
    }

    pub(crate) fn send_ping(&mut self, cx: &mut Cx) {
        if self.pacer.ping(cx.now) {
            cx.send(vec![op::STAT]);
        }
    }

    pub(crate) fn tick(&mut self, cx: &mut Cx) {
        if self.pacer.feed_due(cx.now, FEED_INTERVAL_MS) {
            cx.send(vec![op::FEED]);
        }
        for _ in 0..self.pacer.splits_due(cx.now) {
            cx.send(vec![op::SPLIT]);
        }
    }

    pub(crate) fn next_tick_at(&self) -> Option<f64> {
        self.pacer.next_tick_at()
    }

    pub(crate) fn has_own_cells(&self) -> bool {
        self.cells.alive()
    }

    pub(crate) fn record_policy(&self, data: &[u8]) -> RecordPolicy {
        match data.first() {
            None | Some(0xfe) => RecordPolicy::Skip,
            Some(0x40 | 0x20 | 0x21 | 0x22 | 0x14 | 0x12) => RecordPolicy::Keep,
            Some(_) => RecordPolicy::Normal,
        }
    }
}
