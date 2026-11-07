mod connection;
mod input;
mod join;
mod lifecycle;

pub use connection::ConnInfo;
pub(in crate::world) use connection::MinionSuspect;

use super::cell::Rgb;
use crate::core::math::{Rect, Vec2};
use crate::net::outbox::Outbox;
use crate::net::packets::{Put, Scramble};
use bytes::Bytes;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct Unit {
    pub cells: Vec<u32>,
    pub mouse: Vec2,
    pub color: Rgb,
}

/// A client input, applied in arrival order at the next tick so a split sent
/// before a unit switch still splits the unit that was active.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Mouse(Vec2),
    Split,
    Eject,
    SwitchUnit,
}

/// Inputs kept per tick; older ones are dropped beyond this.
pub const MAX_INPUTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerKind {
    Human,
    Bot,
    Minion { owner_pid: u32, owner: u32 },
}

pub const ROLE_GUEST: u8 = 0;
pub const ROLE_USER: u8 = 1;
pub const ROLE_MODER: u8 = 2;
pub const ROLE_ADMIN: u8 = 4;

pub struct Player {
    pub slot: u32,
    pub pid: u32,
    pub kind: PlayerKind,
    pub conn: Option<Arc<Outbox>>,
    pub ip: String,
    pub protocol: u32,

    pub connected: bool,
    pub removed: bool,
    pub close_requested: bool,
    pub close_time: f64,
    pub last_alive: f64,

    pub name: String,
    pub name_utf8: Vec<u8>,
    pub name_ucs2: Vec<u8>,
    pub skin: String,
    pub skin_utf8: Option<Vec<u8>>,
    pub skin_p11: Option<Vec<u8>>,

    pub units: [Unit; 2],
    pub active: u8,
    pub score: f64,
    pub scale: f64,
    pub center: Vec2,
    pub view_boxes: [Rect; 2],
    pub view_box_count: u8,
    pub active_box: u8,

    pub team: u8,
    pub spectate: bool,
    pub free_roam: bool,
    pub last_keypress_tick: u64,
    pub scramble: Scramble,

    pub is_minion: bool,
    pub role: u8,
    pub user_auth: Option<String>,

    pub spawn_size: f64,
    pub frozen: bool,

    pub minion_split: bool,
    pub minion_eject: bool,
    pub minion_frozen: bool,
    pub has_minions: bool,

    pub last_join_tick: u64,
    pub last_chat_tick: u64,
    pub last_eject: Option<u64>,
    pub split_requested: bool,
    pub eject_requested: bool,
    pub toggle_spectate: bool,
    pub inputs: Vec<Input>,
    pub pending_splits: [usize; 2],
    /// Owned cells are sent with their unit (0x20/0x21) and the active unit
    /// with 0x22, as in xingkong.
    pub unit_packets: bool,
}

impl Player {
    pub fn new(slot: u32, pid: u32, kind: PlayerKind, conn: Option<Arc<Outbox>>) -> Self {
        Self {
            slot,
            pid,
            kind,
            conn,
            ip: String::new(),
            protocol: 0,
            connected: true,
            removed: false,
            close_requested: false,
            close_time: 0.0,
            last_alive: 0.0,
            name: String::new(),
            name_utf8: vec![0],
            name_ucs2: vec![0, 0],
            skin: String::new(),
            skin_utf8: None,
            skin_p11: None,
            units: Default::default(),
            active: 0,
            score: 0.0,
            scale: 1.0,
            center: Vec2::ZERO,
            view_boxes: [Rect::default(); 2],
            view_box_count: 0,
            active_box: 0,
            team: 0,
            spectate: false,
            free_roam: false,
            last_keypress_tick: 0,
            scramble: Scramble::default(),
            is_minion: false,
            role: ROLE_GUEST,
            user_auth: None,
            spawn_size: 0.0,
            frozen: false,
            minion_split: false,
            minion_eject: false,
            minion_frozen: false,
            has_minions: false,
            last_join_tick: 0,
            last_chat_tick: 0,
            last_eject: None,
            split_requested: false,
            eject_requested: false,
            toggle_spectate: false,
            inputs: Vec::new(),
            pending_splits: [0; 2],
            unit_packets: false,
        }
    }

    pub fn is_bot(&self) -> bool {
        self.kind == PlayerKind::Bot
    }

    pub fn is_minion_kind(&self) -> bool {
        matches!(self.kind, PlayerKind::Minion { .. })
    }

    pub fn is_human(&self) -> bool {
        self.kind == PlayerKind::Human
    }

    pub fn cell_count(&self) -> usize {
        self.units[0].cells.len() + self.units[1].cells.len()
    }

    pub fn is_alive(&self) -> bool {
        self.cell_count() != 0
    }

    pub fn active_unit(&self) -> &Unit {
        &self.units[self.active as usize]
    }

    pub fn mouse(&self) -> Vec2 {
        self.active_unit().mouse
    }

    pub fn all_cells(&self) -> impl Iterator<Item = u32> + '_ {
        self.units[0].cells.iter().chain(self.units[1].cells.iter()).copied()
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
        self.name_utf8.clear();
        self.name_utf8.put_utf8z(name);
        self.name_ucs2.clear();
        self.name_ucs2.put_ucs2z(name);
    }

    pub fn set_skin(&mut self, skin: &str) {
        self.skin = skin.to_owned();
        let mut utf8 = Vec::new();
        utf8.put_utf8z(skin);
        self.skin_utf8 = Some(utf8);
        let mut p11 = Vec::new();
        p11.put_utf8z(&format!("%{skin}"));
        self.skin_p11 = Some(p11);
    }

    pub fn send(&self, frame: impl Into<Bytes>) {
        if self.connected {
            if let Some(conn) = &self.conn {
                conn.push(frame);
            }
        }
    }

    pub fn close(&self, code: u16, reason: &str) {
        if let Some(conn) = &self.conn {
            conn.close(code, reason);
        }
    }

    pub fn kick(&mut self, code: u16, reason: &str) {
        self.close(code, reason);
        self.close_requested = true;
    }

    pub fn queue_input(&mut self, input: Input) {
        if self.inputs.len() >= MAX_INPUTS {
            self.inputs.remove(0);
        }
        self.inputs.push(input);
    }

    pub fn set_mouse(&mut self, mouse: Vec2) {
        let active = self.active as usize;
        for (i, unit) in self.units.iter_mut().enumerate() {
            if i == active || unit.cells.is_empty() {
                unit.mouse = mouse;
            }
        }
    }
}

pub fn truncate_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, ch) in s.char_indices() {
        units += ch.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}
