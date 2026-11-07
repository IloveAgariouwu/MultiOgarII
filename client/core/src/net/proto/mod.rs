mod cells;
mod map;
mod ogar;
mod pacer;

use std::collections::VecDeque;

use serde::Deserialize;

use super::hud::HudUpdate;
use crate::world::Tab;

pub(crate) use ogar::Ogar;

pub(crate) const RADIUS_SQ_PER_MASS: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Split(u8),
    ChangeUnit,
    Feed(bool),
    FeedOne,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub name: String,
    pub skin: String,
    pub skin2: String,
    pub external_skin: String,
    pub external_skin2: String,
}

pub struct Cx<'a> {
    pub tab: Tab,
    pub now: f64,
    pub out: &'a mut VecDeque<Vec<u8>>,
    pub hud: &'a mut HudUpdate,
    pub identity: &'a Identity,
    pub publishes: bool,
    pub latency: Option<f64>,
    pub spectating: bool,
}

impl Cx<'_> {
    pub fn send(&mut self, packet: Vec<u8>) {
        self.out.push_back(packet);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordPolicy {
    Skip,
    Normal,
    Keep,
}
