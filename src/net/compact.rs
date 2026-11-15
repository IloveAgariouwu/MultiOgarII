//! The compact protocol: protocol 6 with smaller world updates.
//!
//! A client asks for it by sending protocol 6 and [`HANDSHAKE_KEY`] as the
//! handshake key; protocol 6 servers that do not know it ignore the key. The
//! server answers with [`hello`] as its first frame. After that every message is
//! as in protocol 6, except that the world arrives in [`OP_WORLD`] frames instead
//! of UpdateNodes (0x10), the camera position (0x11) and the owned-cell packets
//! (0x20, 0x21), and players' names and skins arrive once in [`OP_PLAYER`]
//! frames. Messages from the client are unchanged. See `PROTOCOL.md`.

use super::packets::{Put, Scramble};
use crate::world::cell::Rgb;

/// Handshake key that asks for the compact protocol ("MOG1" read as u32 LE).
pub const HANDSHAKE_KEY: u32 = u32::from_le_bytes(*b"MOG1");
pub const VERSION: u8 = 1;

pub const OP_HELLO: u8 = 0x50;
pub const OP_PLAYER: u8 = 0x51;
pub const OP_WORLD: u8 = 0x52;

pub const KIND_FOOD: u8 = 0;
pub const KIND_EJECTED: u8 = 1;
pub const KIND_VIRUS: u8 = 2;
pub const KIND_MOTHER: u8 = 3;
pub const KIND_PLAYER: u8 = 4;
/// Owned cells: `KIND_OWN + unit`.
pub const KIND_OWN: u8 = 5;

pub const CHANGED_X: u8 = 0x01;
pub const CHANGED_Y: u8 = 0x02;
pub const CHANGED_RADIUS: u8 = 0x04;
pub const CHANGED_COLOR: u8 = 0x08;

/// What a client was last told of a cell: its scrambled, truncated position,
/// truncated radius and color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub x: i32,
    pub y: i32,
    pub radius: u32,
    pub color: Rgb,
}

impl Shown {
    pub fn of(x: f64, y: f64, radius: f64, color: Rgb, sc: &Scramble) -> Self {
        Self { x: (x + sc.x) as i32, y: (y + sc.y) as i32, radius: radius as u32, color }
    }
}

/// Scrambles an id by an offset rather than a xor, so ids keep their order and
/// differences and sorted lists of them stay cheap to send.
pub fn scramble_id(id: u32, sc: &Scramble) -> u32 {
    id.wrapping_add(sc.id)
}

pub fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push(v as u8 | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

pub fn put_signed(buf: &mut Vec<u8>, v: i64) {
    put_varint(buf, ((v << 1) ^ (v >> 63)) as u64);
}

fn put_rgb(buf: &mut Vec<u8>, c: Rgb) {
    buf.extend_from_slice(&[c.r, c.g, c.b]);
}

pub fn hello() -> Vec<u8> {
    vec![OP_HELLO, VERSION]
}

pub fn player(pid: u32, name: &str, skin: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + name.len() + skin.len());
    b.put_u8(OP_PLAYER);
    put_varint(&mut b, pid.into());
    b.put_utf8z(name);
    b.put_utf8z(skin);
    b
}

/// Writes a list of ids sorted by scrambled value as differences from the
/// previous one.
#[derive(Default)]
pub struct IdList {
    prev: u32,
}

impl IdList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the difference to the previous id, for callers that pack it with
    /// other bits.
    pub fn step(&mut self, scrambled: u32) -> u64 {
        let delta = scrambled.wrapping_sub(self.prev);
        self.prev = scrambled;
        delta.into()
    }
}

/// The world frame. Sections come in this order, each a varint count followed
/// by its records: eats, updates, adds, removals.
pub struct World {
    pub buf: Vec<u8>,
    camera: (i32, i32),
}

impl World {
    pub fn begin(buf: Vec<u8>, camera_x: f64, camera_y: f64, sc: &Scramble) -> Self {
        let mut w = Self { buf, camera: ((camera_x + sc.x) as i32, (camera_y + sc.y) as i32) };
        w.buf.push(OP_WORLD);
        put_signed(&mut w.buf, w.camera.0.into());
        put_signed(&mut w.buf, w.camera.1.into());
        w
    }

    pub fn count(&mut self, n: usize) {
        put_varint(&mut self.buf, n as u64);
    }

    pub fn eat(&mut self, ids: &mut IdList, prey: u32, hunter: u32) {
        let delta = ids.step(prey);
        put_varint(&mut self.buf, delta);
        put_varint(&mut self.buf, hunter.into());
    }

    pub fn update(&mut self, ids: &mut IdList, id: u32, was: &Shown, now: &Shown) {
        let mut changed = 0;
        changed |= if now.x != was.x { CHANGED_X } else { 0 };
        changed |= if now.y != was.y { CHANGED_Y } else { 0 };
        changed |= if now.radius != was.radius { CHANGED_RADIUS } else { 0 };
        changed |= if now.color != was.color { CHANGED_COLOR } else { 0 };
        put_varint(&mut self.buf, ids.step(id) << 4 | u64::from(changed));
        if changed & CHANGED_X != 0 {
            put_signed(&mut self.buf, i64::from(now.x) - i64::from(was.x));
        }
        if changed & CHANGED_Y != 0 {
            put_signed(&mut self.buf, i64::from(now.y) - i64::from(was.y));
        }
        if changed & CHANGED_RADIUS != 0 {
            put_signed(&mut self.buf, i64::from(now.radius) - i64::from(was.radius));
        }
        if changed & CHANGED_COLOR != 0 {
            put_rgb(&mut self.buf, now.color);
        }
    }

    /// `owner` is the pid of the player that owns a player cell.
    pub fn add(&mut self, ids: &mut IdList, id: u32, kind: u8, now: &Shown, owner: Option<u32>) {
        put_varint(&mut self.buf, ids.step(id) << 3 | u64::from(kind));
        put_signed(&mut self.buf, i64::from(now.x) - i64::from(self.camera.0));
        put_signed(&mut self.buf, i64::from(now.y) - i64::from(self.camera.1));
        put_varint(&mut self.buf, now.radius.into());
        put_rgb(&mut self.buf, now.color);
        if let Some(pid) = owner {
            put_varint(&mut self.buf, pid.into());
        }
    }

    pub fn remove(&mut self, ids: &mut IdList, id: u32) {
        let delta = ids.step(id);
        put_varint(&mut self.buf, delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip_their_byte_counts() {
        let len = |v: u64| {
            let mut b = Vec::new();
            put_varint(&mut b, v);
            b.len()
        };
        assert_eq!([len(0), len(127), len(128), len(16_383), len(16_384)], [1, 1, 2, 2, 3]);
        assert_eq!(len(u32::MAX.into()), 5);
        let signed = |v: i64| {
            let mut b = Vec::new();
            put_signed(&mut b, v);
            b
        };
        assert_eq!(signed(0), [0]);
        assert_eq!(signed(-1), [1]);
        assert_eq!(signed(1), [2]);
        assert_eq!(signed(-64), [127]);
        assert_eq!(signed(64).len(), 2);
    }

    #[test]
    fn id_differences_wrap() {
        let mut ids = IdList::new();
        assert_eq!(ids.step(5), 5);
        assert_eq!(ids.step(9), 4);
        assert_eq!(ids.step(2), u64::from(u32::MAX - 6));
    }
}
