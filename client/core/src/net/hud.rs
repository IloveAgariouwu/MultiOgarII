use serde::Serialize;

use super::tabs::{ControlMode, Intent, TabView};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LeaderRow {
    pub rank: u32,
    pub name: String,
    #[serde(rename = "self")]
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnitRow {
    pub id: u32,
    pub name: String,
    pub mass: f64,
    pub color: String,
    pub cell: String,
    pub spot: MapSpot,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct MapSpot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamRow {
    pub color: String,
    pub share: f64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelfInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split1: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split2: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos2: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Population {
    pub total: u32,
    pub max: u32,
    pub players: u32,
    pub spectators: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatLine {
    pub name: String,
    pub text: String,
    pub color: String,
    pub system: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabsState {
    pub tabs: [TabView; 2],
    pub mode: ControlMode,
    pub tabs_as_units: bool,
    pub active: u8,
    pub intent: Intent,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NetEvent {
    Death { tab: u8 },
    ChatTooLong,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HudUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaderboard: Option<Vec<LeaderRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams: Option<Vec<TeamRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Vec<UnitRow>>,
    #[serde(rename = "self", skip_serializing_if = "Option::is_none")]
    pub self_info: Option<SelfInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub population: Option<Population>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<TabsState>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chat: Vec<ChatLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<NetEvent>,
}

impl HudUpdate {
    pub fn is_empty(&self) -> bool {
        *self == HudUpdate::default()
    }
}

pub fn hex_color(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}
