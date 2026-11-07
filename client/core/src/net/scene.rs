use std::collections::HashMap;

use super::skew::Echo;
use crate::world::{Border, CellUpdate, Kind, Player, ServerView, Sym, Tab, World};

const FIRST_PEER_ID: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerKey {
    pub name: Sym,
    pub skin: Sym,
    pub color: [u8; 3],
}

#[derive(Debug, Default, Clone)]
pub struct Visibility {
    seen: HashMap<u32, u32>,
    pub(crate) echo: Echo,
    peers: HashMap<PeerKey, u32>,
    pub(crate) units: [Option<u32>; 2],
}

impl Visibility {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct Scene<'a> {
    world: &'a mut World,
    vis: &'a mut Visibility,
    tab: Tab,
    now: f64,
}

impl<'a> Scene<'a> {
    pub fn new(world: &'a mut World, vis: &'a mut Visibility, tab: Tab) -> Self {
        Self {
            world,
            vis,
            tab,
            now: 0.0,
        }
    }

    fn bit(&self) -> u32 {
        match self.tab {
            Tab::Main => 1,
            Tab::Second => 2,
        }
    }

    pub fn world(&self) -> &World {
        self.world
    }

    pub fn intern(&mut self, s: &str) -> Sym {
        self.world.intern(s)
    }

    pub fn begin(&mut self, now: f64) {
        self.now = now;
        self.world.begin_update(now);
    }

    pub fn upsert(&mut self, u: CellUpdate) {
        let owner_tab = [Tab::Main, Tab::Second]
            .into_iter()
            .find(|&t| self.world.own(t).contains(&u.id));
        let bit = self.bit();
        let seen = self.vis.seen.entry(u.id).or_default();
        *seen |= bit;
        let drives = match owner_tab {
            Some(tab) => tab == self.tab,
            None => *seen & (bit - 1) == 0,
        };
        if u.kind == Kind::Player {
            self.vis
                .echo
                .report(self.tab, u.id, u.at.x, u.at.y, self.now);
        }
        if drives {
            self.world.upsert(u);
        }
    }

    pub fn remove(&mut self, id: u32, eater: Option<u32>) {
        let bit = self.bit();
        self.world.remove_own(self.tab, id);
        let Some(mask) = self.vis.seen.get_mut(&id) else {
            return;
        };
        *mask &= !bit;
        if *mask == 0 {
            self.vis.seen.remove(&id);
            self.world.remove(id, eater);
        }
    }

    pub fn clear(&mut self) {
        let bit = self.bit();
        self.vis.seen.retain(|_, mask| {
            *mask &= !bit;
            *mask != 0
        });
        if self.vis.seen.is_empty() {
            self.world.clear_cells();
            return;
        }
        let gone: Vec<u32> = self
            .world
            .cells()
            .iter()
            .map(|c| c.id)
            .filter(|id| !self.vis.seen.contains_key(id))
            .collect();
        for id in gone {
            self.world.remove(id, None);
        }
        self.world.clear_own(self.tab);
    }

    pub fn peer(&mut self, key: PeerKey) -> u32 {
        let next = FIRST_PEER_ID + 2 * self.vis.peers.len() as u32;
        *self.vis.peers.entry(key).or_insert(next)
    }

    pub fn set_player(&mut self, id: u32, player: Player) {
        self.world.set_player(id, player);
    }

    pub fn set_controlled_unit(&mut self, unit: Option<u32>) {
        self.vis.units[usize::from(self.tab == Tab::Second)] = unit;
    }

    pub fn set_border(&mut self, border: Border) {
        self.world.set_border(border);
    }

    pub fn add_own(&mut self, id: u32) {
        self.world.add_own(self.tab, id);
    }

    pub fn set_server_view(&mut self, view: Option<ServerView>) {
        let owned =
            !self.world.own(Tab::Main).is_empty() || !self.world.own(Tab::Second).is_empty();
        self.world
            .set_server_view(self.tab, if owned { None } else { view });
    }
}
