use super::outbox::CLOSE_PROTOCOL_ERROR;
use super::packets::uses_ucs2;

pub const OP_JOIN: u8 = 0;
pub const OP_SPECTATE: u8 = 1;
pub const OP_MOUSE: u8 = 16;
pub const OP_SPLIT: u8 = 17;
pub const OP_SPECTATE_TOGGLE: u8 = 18;
pub const OP_EJECT: u8 = 21;
pub const OP_MINION_SPLIT: u8 = 22;
pub const OP_MINION_EJECT: u8 = 23;
pub const OP_MINION_FREEZE: u8 = 24;
pub const OP_SWITCH_UNIT: u8 = 26;
/// Xingkong's Tab: switches unit and turns on the unit packets 0x21 and 0x22.
pub const OP_SWITCH_UNIT_TAGGED: u8 = 44;
pub const OP_CHAT: u8 = 99;
pub const OP_HANDSHAKE_PROTOCOL: u8 = 254;
pub const OP_HANDSHAKE_KEY: u8 = 255;
pub const OP_STAT: u8 = 254;

#[derive(Clone, Debug, PartialEq)]
pub enum ClientMsg {
    Ready { protocol: u32 },
    Join(String),
    Spectate,
    Mouse { x: f64, y: f64 },
    Split,
    SpectateToggle,
    Eject,
    MinionSplit,
    MinionEject,
    MinionFreezeToggle,
    SwitchUnit { tagged: bool },
    Chat(String),
    Stat,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    Msg(ClientMsg),
    Handled,
    Ignored,
    Close(u16, String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Protocol,
    Key { protocol: u32 },
    Playing { protocol: u32 },
}

pub struct Decoder {
    stage: Stage,
}

impl Default for Decoder {
    fn default() -> Self {
        Self { stage: Stage::Protocol }
    }
}

impl Decoder {
    pub fn decode(&mut self, m: &[u8]) -> Decoded {
        let Some(&op) = m.first() else {
            return Decoded::Ignored;
        };
        match self.stage {
            Stage::Protocol => {
                if op != OP_HANDSHAKE_PROTOCOL || m.len() != 5 {
                    return Decoded::Ignored;
                }
                let protocol = i32::from_le_bytes([m[1], m[2], m[3], m[4]]);
                if !(1..=18).contains(&protocol) {
                    return Decoded::Close(CLOSE_PROTOCOL_ERROR, format!("Not supported protocol: {protocol}"));
                }
                self.stage = Stage::Key { protocol: protocol as u32 };
                Decoded::Handled
            }
            Stage::Key { protocol } => {
                if op != OP_HANDSHAKE_KEY || m.len() != 5 {
                    return Decoded::Ignored;
                }
                let key = i32::from_le_bytes([m[1], m[2], m[3], m[4]]);
                if protocol > 6 && key != 0 {
                    return Decoded::Close(CLOSE_PROTOCOL_ERROR, "Not supported protocol".into());
                }
                self.stage = Stage::Playing { protocol };
                Decoded::Msg(ClientMsg::Ready { protocol })
            }
            Stage::Playing { protocol } => match decode_playing(op, m, protocol) {
                Some(msg) => Decoded::Msg(msg),
                None => Decoded::Ignored,
            },
        }
    }
}

fn decode_playing(op: u8, m: &[u8], protocol: u32) -> Option<ClientMsg> {
    let single = m.len() == 1;
    Some(match op {
        OP_JOIN => ClientMsg::Join(read_text(&m[1..], protocol)),
        OP_SPECTATE if single => ClientMsg::Spectate,
        OP_MOUSE => decode_mouse(m)?,
        OP_SPLIT => ClientMsg::Split,
        OP_SPECTATE_TOGGLE if single => ClientMsg::SpectateToggle,
        OP_EJECT if single => ClientMsg::Eject,
        OP_MINION_SPLIT => ClientMsg::MinionSplit,
        OP_MINION_EJECT => ClientMsg::MinionEject,
        OP_MINION_FREEZE => ClientMsg::MinionFreezeToggle,
        OP_SWITCH_UNIT if single => ClientMsg::SwitchUnit { tagged: false },
        OP_SWITCH_UNIT_TAGGED if single => ClientMsg::SwitchUnit { tagged: true },
        OP_CHAT => {
            if m.len() < 3 {
                return None;
            }
            let reserved = (m[1] & 0b1110) as usize * 2;
            if m.len() < 3 + reserved {
                return None;
            }
            ClientMsg::Chat(read_text(&m[2 + reserved..], protocol))
        }
        OP_STAT if single => ClientMsg::Stat,
        _ => return None,
    })
}

fn decode_mouse(m: &[u8]) -> Option<ClientMsg> {
    let (x, y) = match m.len() {
        13 => (
            i32::from_le_bytes([m[1], m[2], m[3], m[4]]) as f64,
            i32::from_le_bytes([m[5], m[6], m[7], m[8]]) as f64,
        ),
        9 => (
            i16::from_le_bytes([m[1], m[2]]) as f64,
            i16::from_le_bytes([m[3], m[4]]) as f64,
        ),
        21 => (
            trunc_double(f64::from_le_bytes(m[1..9].try_into().unwrap())),
            trunc_double(f64::from_le_bytes(m[9..17].try_into().unwrap())),
        ),
        _ => return None,
    };
    Some(ClientMsg::Mouse { x, y })
}

fn trunc_double(v: f64) -> f64 {
    if v.is_finite() {
        (v.trunc() % 4294967296.0) as i64 as u32 as i32 as f64
    } else {
        0.0
    }
}

fn read_text(bytes: &[u8], protocol: u32) -> String {
    if uses_ucs2(protocol) {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .take_while(|&u| u != 0)
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}
