use std::collections::VecDeque;

use super::buf::ReadError;
use super::controls::Controls;
use super::hotkey::Hotkey;
use super::hud::{HudUpdate, NetEvent, TabsState};
use super::proto::{Act, Cx, Identity, Ogar, RecordPolicy, RADIUS_SQ_PER_MASS};
use super::scene::{Scene, Visibility};
use super::skew::Cadence;
use super::tabs::{
    control_mode, other, route, Intent, Route, TabFacts, TabPhase, TabView, UnitMode,
};
use crate::macros::Macro;

mod death;

use self::death::{DeathWatch, LinkSample, Snapshot};
use crate::world::{Control, Kind, Tab, World};

const DEAD_POSITION: &str = "NaN";
const JOIN_HINT_MS: f64 = 3000.0;
const LEFT: u8 = 0;
const RIGHT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseInput {
    pub button: u8,
    pub down: bool,
    pub at: (f32, f32),
}

#[derive(Clone)]
struct Link {
    codec: Ogar,
    out: VecDeque<Vec<u8>>,
    identity: Identity,
    identity_pending: bool,
    last_update: f64,
    updates: u32,
    cadence: Cadence,
    latency: f64,
    controls: Controls,
}

const PARALLEL_AIM_DISTANCE: f32 = 10_000_000.0;

#[derive(Debug, Clone, Copy, Default)]
struct Pointer {
    cursor: (f32, f32),
    real: (f32, f32),
    parallel: bool,
    centers: [Option<(f32, f32)>; 2],
}

#[derive(Debug, Clone, Copy)]
struct CxFlags {
    publishes: bool,
    spectating: bool,
}

impl Link {
    fn split<'a>(
        &'a mut self,
        tab: Tab,
        now: f64,
        hud: &'a mut HudUpdate,
        flags: CxFlags,
    ) -> (&'a mut Controls, &'a mut Ogar, Cx<'a>) {
        let cx = Cx {
            tab,
            now,
            out: &mut self.out,
            hud,
            identity: &self.identity,
            publishes: flags.publishes,
            latency: None,
            spectating: flags.spectating,
        };
        (&mut self.controls, &mut self.codec, cx)
    }
}

#[derive(Clone)]
pub struct Session {
    links: [Option<Link>; 2],
    vis: Visibility,
    hud: HudUpdate,
    chosen: Tab,
    unit_mode: UnitMode,
    views: [TabView; 2],
    intent: Intent,
    requested: Intent,
    dialing: [bool; 2],
    death: DeathWatch,
    joined: [Option<f64>; 2],
    clock: f64,
    published: Option<TabsState>,
    max_score: f64,
    last_self: (f64, u32, u32),
    running: Vec<RunningMacro>,
    pointer: Pointer,
    frozen: bool,
    pressed: Vec<(Hotkey, Tab)>,
}

#[derive(Clone)]
struct RunningMacro {
    name: String,
    queue: VecDeque<(f64, String, bool)>,
    held: Vec<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            links: [None, None],
            vis: Visibility::default(),
            hud: HudUpdate::default(),
            chosen: Tab::Main,
            unit_mode: UnitMode::ServerUnits,
            views: [TabView::default(); 2],
            intent: Intent::Idle,
            requested: Intent::Idle,
            dialing: [false; 2],
            death: DeathWatch::default(),
            joined: [None; 2],
            clock: 0.0,
            published: None,
            max_score: 0.0,
            last_self: (0.0, 0, 0),
            running: Vec::new(),
            pointer: Pointer::default(),
            frozen: true,
            pressed: Vec::new(),
        }
    }
}

impl Session {
    pub fn set_tabs_as_units(&mut self, on: bool) {
        self.unit_mode = if on {
            UnitMode::TabsAsUnits
        } else {
            UnitMode::ServerUnits
        };
    }

    pub fn is_connected(&self, tab: Tab) -> bool {
        self.links[tab.index()].is_some()
    }

    fn publishing_tab(&self) -> Tab {
        if self.is_connected(Tab::Main) {
            Tab::Main
        } else {
            Tab::Second
        }
    }

    fn with_codec<R>(
        &mut self,
        tab: Tab,
        now: f64,
        f: impl FnOnce(&mut Ogar, &mut Cx) -> R,
    ) -> Option<R> {
        self.with_controls(tab, now, |_, p, cx| f(p, cx))
    }

    fn with_controls<R>(
        &mut self,
        tab: Tab,
        now: f64,
        f: impl FnOnce(&mut Controls, &mut Ogar, &mut Cx) -> R,
    ) -> Option<R> {
        let flags = self.cx_flags(tab);
        let link = self.links[tab.index()].as_mut()?;
        let (controls, codec, mut cx) = link.split(tab, now, &mut self.hud, flags);
        Some(f(controls, codec, &mut cx))
    }

    fn cx_flags(&self, tab: Tab) -> CxFlags {
        CxFlags {
            publishes: tab == self.publishing_tab(),
            spectating: self.intent == Intent::Spectate,
        }
    }

    pub fn mark_dialing(&mut self, tab: Tab) {
        self.dialing[tab.index()] = true;
        self.refresh_views();
    }

    pub fn on_open(&mut self, tab: Tab, identity: Identity, now: f64) {
        self.reset_tab(tab);
        self.links[tab.index()] = Some(Link {
            codec: Ogar::default(),
            out: VecDeque::new(),
            identity,
            identity_pending: true,
            last_update: now,
            updates: 0,
            cadence: Cadence::default(),
            latency: 0.0,
            controls: Controls::default(),
        });
        self.with_codec(tab, now, |p, cx| p.on_open(cx));
        self.refresh_views();
    }

    pub fn on_close(&mut self, tab: Tab, world: &mut World) {
        let snapshot = self.link_snapshot(self.clock);
        let confirmed_empty = self.death.tab_confirms_empty(tab, &snapshot);
        self.reset_tab(tab);
        let closed = self.links[tab.index()].take().is_some();
        let last = self.links.iter().all(Option::is_none);
        if closed && !last {
            Scene::new(world, &mut self.vis, tab).clear();
        }
        if closed {
            self.mark_position_dead(tab);
        }
        self.death
            .on_tab_closed(confirmed_empty, self.has_any_cells(), last);
        if last {
            self.frozen = true;
            self.set_control_tab(Tab::Main);
        } else if self.chosen == tab {
            self.set_control_tab(other(tab));
        }
        self.track_score(world);
        self.refresh_views();
    }

    pub fn identity(&self, tab: Tab) -> Option<&Identity> {
        self.links[tab.index()].as_ref().map(|l| &l.identity)
    }

    pub fn set_identity(&mut self, tab: Tab, identity: Identity) {
        if let Some(link) = self.links[tab.index()].as_mut() {
            link.identity = identity;
            link.identity_pending = true;
        }
    }

    pub fn receive(
        &mut self,
        tab: Tab,
        data: &[u8],
        world: &mut World,
        now: f64,
    ) -> Result<(), ReadError> {
        if self.frozen && self.links[tab.index()].is_some() {
            self.frozen = false;
            self.vis.reset();
            world.clear();
        }
        let flags = self.cx_flags(tab);
        let revision = world.revision();
        let Some(link) = self.links[tab.index()].as_mut() else {
            return Ok(());
        };
        if std::mem::take(&mut link.identity_pending) {
            let skins = [
                world.intern(&link.identity.external_skin),
                world.intern(&link.identity.external_skin2),
            ];
            world.set_external_skins(tab, skins);
        }
        let mut scene = Scene::new(world, &mut self.vis, tab);
        let (_, codec, mut cx) = link.split(tab, now, &mut self.hud, flags);
        let result = codec.receive(data, &mut cx, &mut scene);
        if let Some(latency) = cx.latency {
            link.latency = latency;
            match tab {
                Tab::Main => self.hud.latency = Some(latency),
                Tab::Second => self.hud.latency2 = Some(latency),
            }
        }
        self.with_codec(tab, now, |p, cx| p.send_ping(cx));
        self.track_score(world);
        let renewed = self.death.own_cells_replaced(tab, world.own(tab));
        let updated = world.revision() != revision;
        self.on_update(tab, updated, renewed, now);
        self.pointer.centers[tab.index()] = Self::own_center(world, tab);
        if self.pointer.parallel && !self.has_any_cells() {
            self.pointer.parallel = false;
        }
        self.check_deaths(now);
        world.set_parallel_aim(self.pointer.parallel);
        world.set_control(Control {
            tab: self.control_tab(),
            tabs_as_units: self.tabs_as_units(),
        });
        self.show_active_unit(world);
        result
    }

    fn show_active_unit(&self, world: &mut World) {
        if !self.has_any_cells() {
            world.set_active_owner(None);
            return;
        }
        let control = self.control_tab();
        if let Some(unit) = self.vis.units[control.index()].filter(|_| self.has_cells(control)) {
            world.set_active_owner(Some(unit));
        }
    }

    fn track_score(&mut self, world: &World) {
        let own = |tab: Tab| -> &[u32] {
            if self.is_connected(tab) {
                world.own(tab)
            } else {
                &[]
            }
        };
        let mass = |tab: Tab| -> f64 {
            own(tab)
                .iter()
                .filter_map(|id| world.cell(*id))
                .map(|c| f64::from(c.to.r * c.to.r) / RADIUS_SQ_PER_MASS)
                .sum()
        };
        let current = (
            mass(Tab::Main) + mass(Tab::Second),
            own(Tab::Main).len() as u32,
            own(Tab::Second).len() as u32,
        );
        if current == self.last_self {
            return;
        }
        self.last_self = current;
        self.max_score = self.max_score.max(current.0);
        let info = self.hud.self_info.get_or_insert_with(Default::default);
        info.score = Some(current.0);
        info.max = Some(self.max_score);
        info.split1 = Some(current.1);
        info.split2 = Some(current.2);
    }

    fn has_cells(&self, tab: Tab) -> bool {
        self.links[tab.index()]
            .as_ref()
            .is_some_and(|l| l.codec.has_own_cells())
    }

    fn has_any_cells(&self) -> bool {
        self.has_cells(Tab::Main) || self.has_cells(Tab::Second)
    }

    fn on_update(&mut self, tab: Tab, updated: bool, renewed: bool, now: f64) {
        if let Some(link) = self.links[tab.index()].as_mut().filter(|_| updated) {
            link.cadence.record(now - link.last_update);
            link.last_update = now;
            link.updates += 1;
        }
        if self.has_cells(tab) {
            self.joined[tab.index()] = None;
        }
        let snapshot = self.link_snapshot(now);
        self.death.on_update(tab, updated, renewed, &snapshot);
    }

    fn link_snapshot(&mut self, now: f64) -> Snapshot {
        let behind = if self.both_connected() {
            self.vis.echo.behind()
        } else {
            [0.0; 2]
        };
        let links = [Tab::Main, Tab::Second].map(|tab| {
            self.links[tab.index()].as_mut().map(|l| LinkSample {
                present: l.codec.has_own_cells(),
                updates: l.updates,
                last_update: l.last_update,
                interval: l.cadence.interval(),
                latency: l.latency,
            })
        });
        let catch_up = [0, 1]
            .map(|i| links[i].map_or(0, |l: LinkSample| (behind[i] / l.interval).ceil() as u32));
        Snapshot {
            links,
            catch_up,
            now,
        }
    }

    fn on_tab_lost(&mut self, tab: Tab, now: f64) {
        self.with_codec(tab, now, |p, cx| {
            p.send_spectate(cx);
            p.set_free_roam(true, cx);
        });
        self.mark_position_dead(tab);
    }

    fn mark_joining(&mut self, tab: Tab, now: f64) {
        let joined = &mut self.joined[tab.index()];
        if joined.is_none_or(|at| now - at >= JOIN_HINT_MS) {
            *joined = Some(now);
        }
    }

    fn is_joining(&self, tab: Tab) -> bool {
        !self.has_cells(tab)
            && self.joined[tab.index()].is_some_and(|at| self.clock - at < JOIN_HINT_MS)
    }

    fn tab_facts(&self) -> [TabFacts; 2] {
        let spectating = self.requested == Intent::Spectate;
        [Tab::Main, Tab::Second].map(|tab| TabFacts {
            dialing: self.dialing[tab.index()],
            connected: self.is_connected(tab),
            present: self.has_cells(tab),
            joining: self.is_joining(tab),
            dead: self.death.is_dead() || self.death.is_tab_lost(tab),
            spectating,
        })
    }

    fn derive_intent(&self, facts: &[TabFacts; 2]) -> Intent {
        if !self.death.is_out_of_play() || facts.iter().any(|f| f.present || f.joining) {
            Intent::Play
        } else if self.requested == Intent::Spectate {
            Intent::Spectate
        } else {
            Intent::Idle
        }
    }

    fn refresh_views(&mut self) {
        let facts = self.tab_facts();
        self.views = facts.map(TabView::derive);
        self.intent = self.derive_intent(&facts);
    }

    fn check_deaths(&mut self, now: f64) {
        self.clock = now;
        let snapshot = self.link_snapshot(now);
        if self.death.confirm_death(&snapshot) {
            self.on_death(now);
        }
        for tab in [Tab::Main, Tab::Second] {
            if self.death.confirm_tab_lost(tab, &snapshot) {
                self.on_tab_lost(tab, now);
            }
        }
        self.refresh_views();
    }

    fn on_death(&mut self, now: f64) {
        self.reset_max_score();
        self.hud.events.push(NetEvent::Death {
            tab: self.control_tab().index() as u8 + 1,
        });
        for tab in [Tab::Main, Tab::Second] {
            if self.is_connected(tab) {
                self.with_codec(tab, now, |p, cx| p.send_spectate(cx));
                self.mark_position_dead(tab);
            }
        }
    }

    fn reset_tab(&mut self, tab: Tab) {
        let i = tab.index();
        self.dialing[i] = false;
        self.joined[i] = None;
        self.vis.echo.forget(tab);
        self.death.reset_tab(tab);
        self.vis.units[i] = None;
    }

    fn tabs_as_units(&self) -> bool {
        self.unit_mode == UnitMode::TabsAsUnits
    }

    fn is_playing(&self) -> bool {
        let alive = self.views.iter().any(|v| v.phase == TabPhase::Alive);
        let joining = self.views.iter().any(|v| v.phase == TabPhase::Joining);
        self.intent == Intent::Play && (alive || !joining)
    }

    fn mark_position_dead(&mut self, tab: Tab) {
        let info = self.hud.self_info.get_or_insert_with(Default::default);
        match tab {
            Tab::Main => info.pos1 = Some(DEAD_POSITION.to_owned()),
            Tab::Second => info.pos2 = Some(DEAD_POSITION.to_owned()),
        }
    }

    fn reset_max_score(&mut self) {
        self.max_score = 0.0;
        self.hud.self_info.get_or_insert_with(Default::default).max = Some(0.0);
    }

    fn is_aim_frozen(&self) -> bool {
        self.links.iter().flatten().any(|l| l.controls.frozen())
    }

    pub fn aim(&mut self, x: f32, y: f32, now: f64) -> bool {
        let real = (x, y);
        self.pointer.real = real;
        let (x, y) = self.parallel_aim(real);
        self.pointer.cursor = (x, y);
        if self.is_aim_frozen() {
            return false;
        }
        let control = self.control_tab();
        let idle = other(control);
        self.with_codec(control, now, |p, cx| p.aim(x, y, cx));
        if !self.has_cells(idle) {
            self.with_codec(idle, now, |p, cx| p.aim(real.0, real.1, cx));
        }
        true
    }

    pub fn update_pointer(&mut self, aim: (f32, f32)) {
        self.pointer.real = aim;
        self.pointer.cursor = self.parallel_aim(aim);
    }

    pub fn next_tick_at(&self) -> Option<f64> {
        self.links
            .iter()
            .flatten()
            .flat_map(|l| [l.codec.next_tick_at(), l.controls.next_tick_at()])
            .flatten()
            .chain(
                self.running
                    .iter()
                    .filter_map(|r| r.queue.front().map(|s| s.0)),
            )
            .reduce(f64::min)
    }

    pub fn run_macro(&mut self, m: &Macro, now: f64) {
        if let Some(i) = self.running.iter().position(|r| r.name == m.name) {
            let held = self.running.remove(i).held;
            for action in held {
                self.on_hotkey(&action, false, now);
            }
        }
        self.running.push(RunningMacro {
            name: m.name.clone(),
            queue: m
                .timeline()
                .into_iter()
                .map(|(at, action, down)| (now + at, action.to_owned(), down))
                .collect(),
            held: Vec::new(),
        });
        self.run_due_macros(now);
    }

    pub fn stop_macros(&mut self, now: f64) {
        for r in std::mem::take(&mut self.running) {
            for action in r.held {
                self.on_hotkey(&action, false, now);
            }
        }
    }

    fn run_due_macros(&mut self, now: f64) {
        let mut due = Vec::new();
        for r in &mut self.running {
            while let Some((_, action, down)) = r.queue.pop_front_if(|s| s.0 <= now) {
                if down {
                    r.held.push(action.clone());
                } else {
                    r.held.retain(|h| *h != action);
                }
                due.push((action, down));
            }
            if r.queue.is_empty() {
                due.extend(r.held.drain(..).map(|a| (a, false)));
            }
        }
        self.running.retain(|r| !r.queue.is_empty());
        for (action, down) in due {
            self.on_hotkey(&action, down, now);
        }
    }

    fn start_play(&mut self, tab: Tab, now: f64) {
        self.requested = Intent::Play;
        self.set_control_tab(tab);
        self.mark_joining(tab, now);
        self.with_codec(tab, now, |p, cx| p.send_join(cx));
        self.check_deaths(now);
    }

    pub fn join(&mut self, preferred: Option<Tab>, now: f64) {
        let main = self.is_connected(Tab::Main);
        let second = self.is_connected(Tab::Second);
        let alive = [self.has_cells(Tab::Main), self.has_cells(Tab::Second)];
        match (main, second) {
            (true, true) => match alive {
                [false, false] => self.start_play(Tab::Main, now),
                [false, true] if self.tabs_as_units() || preferred != Some(Tab::Second) => {
                    self.start_play(Tab::Main, now)
                }
                [true, false] if self.tabs_as_units() || preferred != Some(Tab::Main) => {
                    self.start_play(Tab::Second, now)
                }
                _ => {}
            },
            (true, false) if !alive[0] => self.start_play(Tab::Main, now),
            (false, true) if !alive[1] => self.start_play(Tab::Second, now),
            _ => {}
        }
    }

    pub fn spectate(&mut self, now: f64) {
        if self.has_any_cells() {
            return;
        }
        self.requested = Intent::Spectate;
        self.death.on_spectate();
        self.joined = [None; 2];
        for tab in [Tab::Main, Tab::Second] {
            self.with_codec(tab, now, |p, cx| p.send_spectate(cx));
        }
        self.check_deaths(now);
    }

    pub fn set_free_roam(&mut self, free: bool, now: f64) {
        for tab in [Tab::Main, Tab::Second] {
            self.with_codec(tab, now, |p, cx| p.set_free_roam(free, cx));
        }
    }

    pub fn send_chat(&mut self, text: &str, now: f64) {
        self.with_codec(self.publishing_tab(), now, |p, cx| p.send_chat(text, cx));
    }

    fn set_control_tab(&mut self, tab: Tab) {
        self.chosen = tab;
    }

    fn control_tab(&self) -> Tab {
        let chosen = self.chosen;
        if self.has_cells(chosen) || self.is_joining(chosen) || !self.has_cells(other(chosen)) {
            chosen
        } else {
            other(chosen)
        }
    }

    fn switch_tab(&mut self, now: f64) {
        if self.both_connected() {
            let from = self.control_tab();
            self.with_codec(from, now, |p, _| p.release_control());
            self.set_control_tab(other(from));
        } else if !self.tabs_as_units() {
            return;
        }
        let chosen = self.chosen;
        if !self.has_cells(chosen) {
            self.join(Some(chosen), now);
        }
    }

    fn change_unit(&mut self, now: f64) {
        let start = if self.tabs_as_units() {
            !self.has_any_cells()
        } else {
            self.control_tab() == Tab::Main && !self.has_cells(Tab::Main)
        };
        if start {
            self.join(Some(Tab::Main), now);
        } else {
            self.press_on_control_tab(Hotkey::ChangeUnit, true, now);
        }
    }

    fn press_on_control_tab(&mut self, hotkey: Hotkey, down: bool, now: f64) -> bool {
        self.press_on(self.control_tab(), hotkey, down, now)
    }

    fn press_on(&mut self, tab: Tab, hotkey: Hotkey, down: bool, now: f64) -> bool {
        let aim = self.pointer.cursor;
        self.with_controls(tab, now, |controls, p, cx| {
            controls.press(hotkey, down, aim, p, cx)
        })
        .unwrap_or(false)
    }

    fn both_connected(&self) -> bool {
        self.is_connected(Tab::Main) && self.is_connected(Tab::Second)
    }

    fn split_control_tab(&mut self, n: u8, now: f64) {
        let control = self.control_tab();
        self.with_codec(control, now, |p, cx| p.send_action(Act::Split(n), cx));
    }

    fn toggle_control_tab(&mut self) {
        self.set_control_tab(other(self.control_tab()));
    }

    pub fn on_hotkey(&mut self, name: &str, down: bool, now: f64) -> bool {
        let Some(hotkey) = Hotkey::parse(name) else {
            return false;
        };
        // Releases always pass, so keys held at death are let go.
        if down && !self.is_playing() && hotkey.playing_only() {
            return false;
        }
        match route(hotkey, down, self.unit_mode, self.both_connected()) {
            Route::Ignore => {}
            Route::SwitchTab => self.switch_tab(now),
            Route::ChangeUnit => self.change_unit(now),
            Route::Join => self.join(None, now),
            Route::TargetLargest => {
                let control = self.control_tab();
                let follows = [control, other(control)]
                    .into_iter()
                    .filter_map(|tab| self.links[tab.index()].as_ref())
                    .map(|l| &l.codec)
                    .find(|p| !p.has_own_cells())
                    .is_some_and(|p| p.is_following_largest());
                self.set_free_roam(follows, now);
            }
            Route::SplitAcross(first, second) => {
                self.split_control_tab(first, now);
                if self.has_cells(other(self.control_tab())) {
                    self.toggle_control_tab();
                    let control = self.control_tab();
                    // The parallel aim depends on the controlled tab's center.
                    let (x, y) = self.parallel_aim(self.pointer.real);
                    self.pointer.cursor = (x, y);
                    self.with_codec(control, now, |p, cx| p.aim(x, y, cx));
                    self.split_control_tab(second, now);
                }
            }
            Route::OverlapSplit => {
                self.split_control_tab(4, now);
                if self.has_cells(other(self.control_tab())) {
                    self.toggle_control_tab();
                    self.split_control_tab(4, now);
                }
                let control = self.control_tab();
                self.with_controls(control, now, |controls, p, cx| controls.overlap_feed(p, cx));
            }
            Route::OverlapSplitOnce => {
                self.split_control_tab(1, now);
                if self.has_cells(other(self.control_tab())) {
                    self.toggle_control_tab();
                    self.split_control_tab(1, now);
                    self.toggle_control_tab();
                }
            }
            Route::Parallel => self.parallel(down, now),
            Route::Active => {
                let held = self.pressed.iter().position(|&(h, _)| h == hotkey);
                let held = held.map(|i| self.pressed.swap_remove(i).1);
                let tab = if down {
                    let tab = self.control_tab();
                    self.pressed.push((hotkey, tab));
                    tab
                } else {
                    held.unwrap_or_else(|| self.control_tab())
                };
                let handled = self.press_on(tab, hotkey, down, now);
                self.check_deaths(now);
                return handled;
            }
        }
        self.check_deaths(now);
        true
    }

    fn player_at(world: &World, x: f32, y: f32) -> bool {
        world
            .cells()
            .iter()
            .any(|c| c.kind == Kind::Player && (c.to.x - x).hypot(c.to.y - y) < c.to.r)
    }

    fn parallel(&mut self, down: bool, now: f64) {
        let on = down && self.pointer.centers[self.control_tab().index()].is_some();
        if on == self.pointer.parallel {
            return;
        }
        self.pointer.parallel = on;
        let (x, y) = self.pointer.real;
        self.aim(x, y, now);
    }

    fn parallel_aim(&self, real: (f32, f32)) -> (f32, f32) {
        let center = self.pointer.centers[self.control_tab().index()];
        let Some((cx, cy)) = center.filter(|_| self.pointer.parallel) else {
            return real;
        };
        let (dx, dy) = (real.0 - cx, real.1 - cy);
        let len = dx.hypot(dy);
        if len == 0.0 {
            return real;
        }
        (
            cx + dx / len * PARALLEL_AIM_DISTANCE,
            cy + dy / len * PARALLEL_AIM_DISTANCE,
        )
    }

    fn own_center(world: &World, tab: Tab) -> Option<(f32, f32)> {
        let (sx, sy, n) = world
            .own(tab)
            .iter()
            .filter_map(|&id| world.cell(id))
            .fold((0.0, 0.0, 0.0), |(sx, sy, n), c| {
                (sx + c.to.x, sy + c.to.y, n + 1.0)
            });
        (n > 0.0).then(|| (sx / n, sy / n))
    }

    pub fn on_mouse(&mut self, input: MouseInput, world: &mut World, now: f64) {
        let MouseInput { button, down, at } = input;
        if !down {
            return;
        }
        let free = match button {
            RIGHT => !Self::player_at(world, at.0, at.1),
            LEFT => true,
            _ => return,
        };
        self.set_free_roam(free, now);
    }

    pub fn tick(&mut self, now: f64) {
        self.run_due_macros(now);
        for tab in [Tab::Main, Tab::Second] {
            self.with_controls(tab, now, |controls, p, cx| {
                p.tick(cx);
                controls.tick(p, cx);
            });
        }
        self.check_deaths(now);
    }

    pub fn record_policy(&self, tab: Tab, data: &[u8]) -> RecordPolicy {
        self.links[tab.index()]
            .as_ref()
            .map_or(RecordPolicy::Skip, |l| l.codec.record_policy(data))
    }

    pub fn take_outgoing(&mut self, tab: Tab) -> Option<Vec<u8>> {
        self.links[tab.index()].as_mut()?.out.pop_front()
    }

    pub fn take_hud(&mut self) -> Option<HudUpdate> {
        let state = TabsState {
            tabs: self.views,
            mode: control_mode(self.views, self.unit_mode),
            tabs_as_units: self.tabs_as_units(),
            active: self.control_tab().index() as u8,
            intent: self.intent,
        };
        if self.published.as_ref() != Some(&state) {
            self.published = Some(state.clone());
            self.hud.tabs = Some(state);
        }
        if self.hud.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.hud))
    }
}
