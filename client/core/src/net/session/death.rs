use crate::net::tabs::other;
use crate::world::Tab;

const CONFIRM_UPDATES: u32 = 6;
const MAX_CONFIRM_UPDATES: u32 = 25;
const LEARN_MARGIN_UPDATES: u32 = 2;
const SILENT_UPDATES: f64 = 10.0;
const SILENT_MIN_MS: f64 = 1000.0;
const SILENT_MAX_MS: f64 = 3000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LinkSample {
    pub(super) present: bool,
    pub(super) updates: u32,
    pub(super) last_update: f64,
    pub(super) interval: f64,
    pub(super) latency: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Snapshot {
    pub(super) links: [Option<LinkSample>; 2],
    pub(super) catch_up: [u32; 2],
    pub(super) now: f64,
}

impl Snapshot {
    fn link(&self, tab: Tab) -> Option<&LinkSample> {
        self.links[tab.index()].as_ref()
    }

    pub(super) fn has_cells(&self, tab: Tab) -> bool {
        self.link(tab).is_some_and(|l| l.present)
    }

    pub(super) fn any_has_cells(&self) -> bool {
        self.has_cells(Tab::Main) || self.has_cells(Tab::Second)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Life {
    Out,
    In,
    Fading { since: f64, counts: [u32; 2] },
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Presence {
    Out,
    In,
    Fading {
        since: f64,
        emptied: u32,
        counts: [u32; 2],
    },
    Lost,
}

#[derive(Debug, Clone)]
pub(super) struct DeathWatch {
    life: Life,
    presence: [Presence; 2],
    arrivals: [Option<u32>; 2],
    owned: [Vec<u32>; 2],
    confirm: u32,
}

impl Default for DeathWatch {
    fn default() -> Self {
        Self {
            life: Life::Out,
            presence: [Presence::Out; 2],
            arrivals: [None; 2],
            owned: [Vec::new(), Vec::new()],
            confirm: CONFIRM_UPDATES,
        }
    }
}

fn went_silent(link: &LinkSample, since: f64, now: f64) -> bool {
    let silence = (SILENT_UPDATES * link.interval).clamp(SILENT_MIN_MS, SILENT_MAX_MS);
    now - link.last_update.max(since) > silence
}

impl DeathWatch {
    pub(super) fn is_dead(&self) -> bool {
        self.life == Life::Dead
    }

    pub(super) fn is_tab_lost(&self, tab: Tab) -> bool {
        self.presence[tab.index()] == Presence::Lost
    }

    pub(super) fn is_out_of_play(&self) -> bool {
        matches!(self.life, Life::Out | Life::Dead)
    }

    pub(super) fn own_cells_replaced(&mut self, tab: Tab, own: &[u32]) -> bool {
        let before = &mut self.owned[tab.index()];
        let renewed = !own.is_empty() && !own.iter().any(|id| before.contains(id));
        before.clear();
        before.extend_from_slice(own);
        renewed
    }

    pub(super) fn on_update(&mut self, tab: Tab, updated: bool, renewed: bool, s: &Snapshot) {
        self.update_tab_presence(tab, updated, renewed, s);
        if s.any_has_cells() {
            if let Life::Fading { counts, .. } = self.life {
                let survived = [Tab::Main, Tab::Second]
                    .into_iter()
                    .filter(|&t| s.link(t).is_some())
                    .map(|t| counts[t.index()].saturating_sub(s.catch_up[t.index()]))
                    .max()
                    .expect("cells mean a connected tab");
                self.widen_confirm_window(survived);
            }
            self.life = Life::In;
            return;
        }
        match &mut self.life {
            Life::Out | Life::Dead => {}
            Life::In => {
                self.life = Life::Fading {
                    since: s.now,
                    counts: [0; 2],
                }
            }
            Life::Fading { counts, .. } if updated => counts[tab.index()] += 1,
            Life::Fading { .. } => {}
        }
    }

    fn widen_confirm_window(&mut self, survived: u32) {
        self.confirm = self
            .confirm
            .max(survived + LEARN_MARGIN_UPDATES)
            .min(MAX_CONFIRM_UPDATES);
    }

    fn update_tab_presence(&mut self, tab: Tab, updated: bool, renewed: bool, s: &Snapshot) {
        let i = tab.index();
        let o = 1 - i;
        let present = s.has_cells(tab);
        if let Presence::Fading { counts, .. } = &mut self.presence[o] {
            counts[i] += u32::from(updated);
        }
        match (&mut self.presence[i], present) {
            (Presence::In, true) => {
                if renewed {
                    self.on_tab_cells_appeared(tab, s);
                }
            }
            (Presence::In, false) => {
                let emptied = s.link(tab).expect("the receiving link").updates;
                self.presence[i] = Presence::Fading {
                    since: s.now,
                    emptied,
                    counts: [0; 2],
                };
            }
            (Presence::Fading { counts, .. }, true) => {
                let survived = counts[i];
                self.widen_confirm_window(survived.saturating_sub(s.catch_up[i]));
                self.on_tab_cells_appeared(tab, s);
            }
            (Presence::Out | Presence::Lost, true) => self.on_tab_cells_appeared(tab, s),
            (Presence::Fading { counts, .. }, false) => counts[i] += u32::from(updated),
            (Presence::Out | Presence::Lost, false) => {}
        }
    }

    fn on_tab_cells_appeared(&mut self, tab: Tab, s: &Snapshot) {
        let i = tab.index();
        let o = 1 - i;
        self.presence[i] = Presence::In;
        self.arrivals[i] = None;
        match self.presence[o] {
            Presence::In => {
                self.arrivals[i] = s.link(other(tab)).map(|l| {
                    let overdue = ((s.now - l.last_update) / l.interval).floor() as u32;
                    l.updates + overdue
                });
            }
            Presence::Fading { .. } => self.presence[o] = Presence::Out,
            Presence::Out | Presence::Lost => {}
        }
    }

    pub(super) fn tab_confirms_empty(&self, tab: Tab, s: &Snapshot) -> bool {
        let Life::Fading { since, counts } = self.life else {
            return true;
        };
        let i = tab.index();
        let window = self.confirm + s.catch_up[i];
        s.link(tab)
            .is_none_or(|l| counts[i] >= window || went_silent(l, since, s.now))
    }

    fn all_tabs_confirm_empty(&self, s: &Snapshot) -> bool {
        matches!(self.life, Life::Fading { .. })
            && [Tab::Main, Tab::Second]
                .into_iter()
                .all(|tab| self.tab_confirms_empty(tab, s))
    }

    #[must_use]
    pub(super) fn confirm_death(&mut self, s: &Snapshot) -> bool {
        if !self.all_tabs_confirm_empty(s) {
            return false;
        }
        self.life = Life::Dead;
        self.presence = [Presence::Out; 2];
        true
    }

    fn latency_behind(tab: Tab, s: &Snapshot) -> u32 {
        let latency = |t: Tab| s.link(t).map_or(0.0, |l| l.latency);
        let lead = (latency(tab) - latency(other(tab))) / 2.0;
        s.link(tab)
            .map_or(0, |l| (lead / l.interval).ceil().max(0.0) as u32)
    }

    #[must_use]
    pub(super) fn confirm_tab_lost(&mut self, tab: Tab, s: &Snapshot) -> bool {
        let i = tab.index();
        let o = 1 - i;
        let Presence::Fading {
            since,
            emptied,
            counts,
        } = self.presence[i]
        else {
            return false;
        };
        let confirm = self.confirm;
        let behind = s.catch_up[i].max(Self::latency_behind(tab, s));
        let other_behind = s.catch_up[o].max(Self::latency_behind(other(tab), s));
        if self.arrivals[o].is_some_and(|at| emptied.saturating_sub(at) <= confirm + behind) {
            self.presence[i] = Presence::Out;
            return false;
        }
        let other_confirmed_empty = s
            .link(other(tab))
            .is_some_and(|l| counts[o] > other_behind || went_silent(l, since, s.now));
        let lost = s.has_cells(other(tab))
            && other_confirmed_empty
            && s.link(tab).is_some_and(|l| {
                counts[i] >= confirm + s.catch_up[i] || went_silent(l, since, s.now)
            });
        if lost {
            self.presence[i] = Presence::Lost;
        }
        lost
    }

    pub(super) fn on_tab_closed(&mut self, confirmed_empty: bool, any_present: bool, last: bool) {
        if (self.life == Life::In || !confirmed_empty) && !any_present {
            self.life = Life::Out;
        }
        if last {
            self.life = Life::Out;
        }
    }

    pub(super) fn reset_tab(&mut self, tab: Tab) {
        let i = tab.index();
        if let Life::Fading { counts, .. } = &mut self.life {
            counts[i] = 0;
        }
        self.presence[i] = Presence::Out;
        self.arrivals = [None; 2];
        self.owned[i].clear();
        if let Presence::Fading { counts, .. } = &mut self.presence[1 - i] {
            counts[i] = 0;
        }
    }

    pub(super) fn on_spectate(&mut self) {
        if self.life == Life::Dead {
            self.life = Life::Out;
        }
    }
}
