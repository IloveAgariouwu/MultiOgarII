use crate::world::cell::{Kind, Rgb};

pub const OP_UPDATE_NODES: u8 = 0x10;
pub const OP_UPDATE_POSITION: u8 = 0x11;
pub const OP_CLEAR_ALL: u8 = 0x12;
pub const OP_CLEAR_OWNED: u8 = 0x14;
pub const OP_ADD_NODE: u8 = 0x20;
pub const OP_ACTIVE_UNIT: u8 = 0x22;
pub const OP_SET_BORDER: u8 = 0x40;
pub const OP_CHAT: u8 = 0x63;
pub const OP_STAT: u8 = 0xFE;
pub const OP_LEADERBOARD_FFA: u8 = 0x31;
pub const OP_LEADERBOARD_TEAMS: u8 = 0x32;
pub const OP_LEADERBOARD_RICH: u8 = 0x33;
pub const OP_LEADERBOARD_RICH_P14: u8 = 0x35;

const NODE_FLAG_VIRUS: u8 = 0x01;
const NODE_FLAG_COLOR: u8 = 0x02;
const NODE_FLAG_SKIN: u8 = 0x04;
const NODE_FLAG_NAME: u8 = 0x08;
const NODE_FLAG_EJECTED: u8 = 0x20;
const NODE_FLAG_EXTENDED: u8 = 0x80;
const EXTENDED_FLAG_FOOD: u8 = 0x01;

const PROTOCOL_32BIT_COORDS: u32 = 5;
const PROTOCOL_V4_NODE_SKINS: u32 = 5;
const PROTOCOL_UTF8_TEXT: u32 = 6;
const PROTOCOL_V6_NODES: u32 = 6;
pub const PROTOCOL_P11: u32 = 11;
const PROTOCOL_P14: u32 = 14;

pub fn uses_ucs2(protocol: u32) -> bool {
    protocol < PROTOCOL_UTF8_TEXT
}

pub trait Put {
    fn put_u8(&mut self, v: u8);
    fn put_u16(&mut self, v: u16);
    fn put_u32(&mut self, v: u32);
    fn put_f32(&mut self, v: f32);
    fn put_f64(&mut self, v: f64);
    fn put_utf8z(&mut self, s: &str);
    fn put_ucs2z(&mut self, s: &str);
}

impl Put for Vec<u8> {
    fn put_u8(&mut self, v: u8) {
        self.push(v);
    }
    fn put_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_f32(&mut self, v: f32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_f64(&mut self, v: f64) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_utf8z(&mut self, s: &str) {
        self.extend_from_slice(s.as_bytes());
        self.push(0);
    }
    fn put_ucs2z(&mut self, s: &str) {
        for unit in s.encode_utf16() {
            self.put_u16(unit);
        }
        self.put_u16(0);
    }
}

fn put_text(buf: &mut Vec<u8>, protocol: u32, s: &str) {
    if uses_ucs2(protocol) {
        buf.put_ucs2z(s);
    } else {
        buf.put_utf8z(s);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Scramble {
    pub x: f64,
    pub y: f64,
    pub id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeRecord {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub color: Rgb,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NodeLabel<'a> {
    pub skin: Option<&'a [u8]>,
    pub name: Option<&'a [u8]>,
}

pub fn begin_update_nodes(buf: &mut Vec<u8>, sc: &Scramble, eats: &[(u32, u32)]) {
    buf.put_u8(OP_UPDATE_NODES);
    buf.put_u16(eats.len() as u16);
    for &(hunter, prey) in eats {
        buf.put_u32(hunter ^ sc.id);
        buf.put_u32(prey ^ sc.id);
    }
}

pub fn put_node(
    buf: &mut Vec<u8>,
    protocol: u32,
    sc: &Scramble,
    node: &NodeRecord,
    label: Option<NodeLabel>,
) {
    let x = (node.x + sc.x) as i32;
    let y = (node.y + sc.y) as i32;

    buf.put_u32(node.id ^ sc.id);
    if protocol < PROTOCOL_32BIT_COORDS {
        buf.put_u16(x as u16);
        buf.put_u16(y as u16);
    } else {
        buf.put_u32(x as u32);
        buf.put_u32(y as u32);
    }
    buf.put_u16(node.radius as u32 as u16);

    let mut flags = 0u8;
    if node.kind == Kind::Virus {
        flags |= NODE_FLAG_VIRUS;
    }
    if node.kind == Kind::Ejected {
        flags |= NODE_FLAG_EJECTED;
    }

    if protocol < PROTOCOL_V6_NODES {
        put_node_v4(buf, protocol, node, flags, label);
    } else {
        put_node_v6(buf, protocol, node, flags, label);
    }
}

fn put_node_v4(
    buf: &mut Vec<u8>,
    protocol: u32,
    node: &NodeRecord,
    mut flags: u8,
    label: Option<NodeLabel>,
) {
    let skin = if protocol == PROTOCOL_V4_NODE_SKINS { label.and_then(|l| l.skin) } else { None };
    if skin.is_some() {
        flags |= NODE_FLAG_SKIN;
    }
    buf.extend_from_slice(&[node.color.r, node.color.g, node.color.b, flags]);
    if let Some(skin) = skin {
        buf.extend_from_slice(skin);
    }
    match label.and_then(|l| l.name) {
        Some(name) => buf.extend_from_slice(name),
        None => buf.put_u16(0),
    }
}

fn put_node_v6(
    buf: &mut Vec<u8>,
    protocol: u32,
    node: &NodeRecord,
    mut flags: u8,
    label: Option<NodeLabel>,
) {
    if label.is_some() || node.kind == Kind::Player {
        flags |= NODE_FLAG_COLOR;
    }
    let (skin, name) = label.map_or((None, None), |l| (l.skin, l.name));
    if skin.is_some() {
        flags |= NODE_FLAG_SKIN;
    }
    if name.is_some() {
        flags |= NODE_FLAG_NAME;
    }
    if protocol >= PROTOCOL_P11 && node.kind == Kind::Food {
        flags |= NODE_FLAG_EXTENDED;
    }
    buf.put_u8(flags);
    if flags & NODE_FLAG_EXTENDED != 0 {
        buf.put_u8(EXTENDED_FLAG_FOOD);
    }
    if flags & NODE_FLAG_COLOR != 0 {
        buf.extend_from_slice(&[node.color.r, node.color.g, node.color.b]);
    }
    if let Some(skin) = skin {
        buf.extend_from_slice(skin);
    }
    if let Some(name) = name {
        buf.extend_from_slice(name);
    }
}

pub fn end_nodes(buf: &mut Vec<u8>) {
    buf.put_u32(0);
}

pub fn put_remove_count(buf: &mut Vec<u8>, protocol: u32, count: usize) {
    if protocol < PROTOCOL_V6_NODES {
        buf.put_u32(count as u32);
    } else {
        buf.put_u16(count as u16);
    }
}

pub fn put_removed(buf: &mut Vec<u8>, sc: &Scramble, id: u32) {
    buf.put_u32(id ^ sc.id);
}

pub fn update_position(x: f64, y: f64, scale: f64, sc: &Scramble) -> Vec<u8> {
    let mut b = Vec::with_capacity(13);
    b.put_u8(OP_UPDATE_POSITION);
    b.put_f32((x + sc.x) as f32);
    b.put_f32((y + sc.y) as f32);
    b.put_f32(scale as f32);
    b
}

pub fn clear_all() -> Vec<u8> {
    vec![OP_CLEAR_ALL]
}

pub fn clear_owned() -> Vec<u8> {
    vec![OP_CLEAR_OWNED]
}

pub fn add_node(id: u32, sc: &Scramble) -> Vec<u8> {
    let mut b = Vec::with_capacity(5);
    b.put_u8(OP_ADD_NODE);
    b.put_u32(id ^ sc.id);
    b
}

/// Like xingkong: 0x20 for a cell of unit 0 and 0x21 for a cell of unit 1.
pub fn add_unit_node(id: u32, unit: u8, sc: &Scramble) -> Vec<u8> {
    let mut b = Vec::with_capacity(5);
    b.put_u8(OP_ADD_NODE + unit);
    b.put_u32(id ^ sc.id);
    b
}

pub fn active_unit(unit: u8) -> Vec<u8> {
    vec![OP_ACTIVE_UNIT, unit]
}

pub fn set_border(
    border: [f64; 4],
    sc: &Scramble,
    game: Option<(u32, &str)>,
    protocol: u32,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    b.put_u8(OP_SET_BORDER);
    b.put_f64(border[0] + sc.x);
    b.put_f64(border[1] + sc.y);
    b.put_f64(border[2] + sc.x);
    b.put_f64(border[3] + sc.y);
    if let Some((game_type, name)) = game {
        b.put_u32(game_type);
        put_text(&mut b, protocol, name);
    }
    b
}

pub fn chat(flags: u8, color: Rgb, name: &str, text: &str, protocol: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + name.len() + text.len());
    b.put_u8(OP_CHAT);
    b.put_u8(flags);
    b.extend_from_slice(&[color.r, color.g, color.b]);
    put_text(&mut b, protocol, name);
    put_text(&mut b, protocol, text);
    b
}

pub fn server_stat(json: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(json.len() + 2);
    b.put_u8(OP_STAT);
    b.put_utf8z(json);
    b
}

#[derive(Clone, Copy, Debug)]
pub struct LbRow<'a> {
    pub name: &'a str,
    pub name_utf8: &'a [u8],
    pub name_ucs2: &'a [u8],
    pub is_me: bool,
    pub my_cell: u32,
}

pub fn leaderboard_ffa(
    rows: &[LbRow],
    protocol: u32,
    sc: &Scramble,
    place: usize,
    alive: bool,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(32 + rows.len() * 24);
    if protocol < PROTOCOL_P11 {
        b.put_u8(OP_LEADERBOARD_FFA);
        b.put_u32(rows.len() as u32);
        for row in rows {
            if uses_ucs2(protocol) {
                let id = if row.my_cell != 0 { row.my_cell ^ sc.id } else { 0 };
                b.put_u32(id);
                b.extend_from_slice(row.name_ucs2);
            } else {
                b.put_u32(row.is_me as u32);
                b.extend_from_slice(row.name_utf8);
            }
        }
        return b;
    }
    b.put_u8(if protocol < PROTOCOL_P14 { OP_LEADERBOARD_RICH } else { OP_LEADERBOARD_RICH_P14 });
    for row in rows {
        if row.is_me {
            b.put_u8(0x09);
            b.put_u16(1);
        } else {
            b.put_u8(0x02);
            b.put_utf8z(row.name);
        }
    }
    if alive && place > 10 {
        b.put_u8(0x09);
        b.put_u16(place as u16);
    }
    b
}

pub fn leaderboard_teams(shares: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(5 + shares.len() * 4);
    b.put_u8(OP_LEADERBOARD_TEAMS);
    b.put_u32(shares.len() as u32);
    for &share in shares {
        b.put_f32(share);
    }
    b
}
