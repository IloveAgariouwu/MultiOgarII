mod interner;
pub mod preview;
pub mod sample;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use interner::Interner;
pub use interner::Sym;

static NEXT_WORLD_ID: AtomicU64 = AtomicU64::new(1);

const RADIUS_SQ_PER_MASS: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Player,
    Food,
    Virus,
    Mother,
    Eject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circle {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

impl Circle {
    pub fn mass(self) -> f32 {
        self.r * self.r / RADIUS_SQ_PER_MASS
    }

    pub fn radius_of(mass: f32) -> f32 {
        (mass * RADIUS_SQ_PER_MASS).sqrt()
    }

    pub fn lerp(self, to: Circle, t: f32) -> Circle {
        Circle {
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
            r: self.r + (to.r - self.r) * t,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellUpdate {
    pub id: u32,
    pub kind: Kind,
    pub at: Circle,
    pub color: Rgb,
    pub owner: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub id: u32,
    pub kind: Kind,
    pub color: Rgb,
    pub owner: u32,
    pub from: Circle,
    pub to: Circle,
    pub updated: f64,
    pub velocity: (f32, f32),
}

impl Cell {
    pub fn at(&self, now: f64, lerp_ms: f64) -> Circle {
        let t = if lerp_ms > 0.0 {
            ((now - self.updated) / lerp_ms).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        self.from.lerp(self.to, t)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eaten {
    pub cell: Cell,
    pub eater: Option<u32>,
    pub at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapDot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub color: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Border {
    pub fn is_valid(&self) -> bool {
        self.max_x > self.min_x && self.max_y > self.min_y
    }

    pub fn center(&self) -> (f32, f32) {
        (
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub name: Sym,
    pub skins: [Sym; 2],
    pub team_color: Rgb,
}

pub fn player_key(owner: u32) -> u32 {
    owner & !1
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aim {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub splitting: bool,
}

pub const AUTO_SPLIT_ALERT_MASS: f32 = 17325.0;
const WAVE_MS: f64 = 3000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wave {
    pub x: f32,
    pub y: f32,
    pub at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServerView {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    #[default]
    Main,
    Second,
}

impl Tab {
    pub(crate) fn index(self) -> usize {
        match self {
            Tab::Main => 0,
            Tab::Second => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Control {
    pub tab: Tab,
    pub tabs_as_units: bool,
}

#[derive(Debug, Clone, Copy)]
struct Presentation {
    id: u64,
    epoch: u64,
    lerp_ms: f64,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            epoch: 0,
            lerp_ms: 120.0,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct World {
    presentation: Presentation,
    cells: Vec<Cell>,
    slots: HashMap<u32, usize>,
    eaten: Vec<Eaten>,
    strings: Interner,
    border: Border,
    own: [Vec<u32>; 2],
    server_views: [Option<ServerView>; 2],
    now: f64,
    revision: u64,
    food_revision: u64,
    food_moved_at: Option<f64>,
    players: HashMap<u32, Player>,
    active_owner: Option<u32>,
    parallel_aim: bool,
    spectate_aim: Option<Aim>,
    external_skins: [[Sym; 2]; 2],
    waves: Vec<Wave>,
    control: Control,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(&self) -> u64 {
        self.presentation.id
    }

    pub fn epoch(&self) -> u64 {
        self.presentation.epoch
    }

    pub fn intern(&mut self, s: &str) -> Sym {
        self.strings.intern(s)
    }

    pub fn string(&self, sym: Sym) -> &str {
        self.strings.get(sym)
    }

    pub fn begin_update(&mut self, now: f64) {
        self.now = now;
        self.revision += 1;
        let keep = self.presentation.lerp_ms;
        self.eaten.retain(|e| now - e.at < keep);
        self.waves.retain(|w| now - w.at < WAVE_MS);
    }

    pub fn set_lerp_ms(&mut self, ms: f64) {
        self.presentation.lerp_ms = ms;
    }

    pub fn lerp_ms(&self) -> f64 {
        self.presentation.lerp_ms
    }

    pub fn upsert(&mut self, u: CellUpdate) {
        let now = self.now;
        match self.slots.get(&u.id) {
            Some(&slot) => {
                let lerp = self.presentation.lerp_ms;
                let c = &mut self.cells[slot];
                if u.kind == Kind::Food && c.to != u.at {
                    self.food_revision += 1;
                    self.food_moved_at = Some(now);
                } else if u.kind == Kind::Food && c.color != u.color {
                    self.food_revision += 1;
                }
                c.velocity = (u.at.x - c.to.x, u.at.y - c.to.y);
                c.from = c.at(now, lerp);
                c.to = u.at;
                c.updated = now;
                c.kind = u.kind;
                c.color = u.color;
                c.owner = u.owner;
            }
            None => {
                if u.kind == Kind::Food {
                    self.food_revision += 1;
                }
                self.slots.insert(u.id, self.cells.len());
                self.cells.push(Cell {
                    id: u.id,
                    kind: u.kind,
                    color: u.color,
                    owner: u.owner,
                    velocity: (0.0, 0.0),
                    from: u.at,
                    to: u.at,
                    updated: now,
                });
            }
        }
    }

    pub fn remove(&mut self, id: u32, eater: Option<u32>) {
        let Some(slot) = self.slots.remove(&id) else {
            return;
        };
        let cell = self.cells.swap_remove(slot);
        if cell.kind == Kind::Food {
            self.food_revision += 1;
        }
        if let Some(moved) = self.cells.get(slot) {
            self.slots.insert(moved.id, slot);
        }
        for own in &mut self.own {
            own.retain(|&o| o != id);
        }
        self.eaten.push(Eaten {
            cell,
            eater,
            at: self.now,
        });
    }

    pub fn clear(&mut self) {
        self.clear_cells();
        self.players.clear();
        self.border = Border::default();
        self.external_skins = Default::default();
        self.food_moved_at = None;
        self.presentation.epoch += 1;
    }

    pub fn restore(&mut self, snapshot: &World) {
        let presentation = self.presentation;
        let revision = self.revision.max(snapshot.revision) + 1;
        let food_revision = self.food_revision.max(snapshot.food_revision) + 1;
        self.clone_from(snapshot);
        self.presentation = Presentation {
            epoch: presentation.epoch + 1,
            ..presentation
        };
        self.revision = revision;
        self.food_revision = food_revision;
    }

    pub fn settle(&mut self) {
        for c in &mut self.cells {
            c.from = c.to;
        }
        self.eaten.clear();
        self.waves.clear();
        self.revision += 1;
    }

    pub fn clear_cells(&mut self) {
        self.cells.clear();
        self.slots.clear();
        self.eaten.clear();
        self.own.iter_mut().for_each(Vec::clear);
        self.server_views = [None; 2];
        self.active_owner = None;
        self.spectate_aim = None;
        self.parallel_aim = false;
        self.waves.clear();
        self.revision += 1;
        self.food_revision += 1;
    }

    pub fn set_player(&mut self, id: u32, player: Player) {
        self.players.insert(player_key(id), player);
    }

    pub fn player(&self, owner: u32) -> Option<&Player> {
        self.players.get(&player_key(owner))
    }

    pub fn set_active_owner(&mut self, owner: Option<u32>) {
        self.active_owner = owner;
    }

    pub fn active_owner(&self) -> Option<u32> {
        self.active_owner
    }

    pub fn set_spectate_aim(&mut self, aim: Option<Aim>) {
        self.spectate_aim = aim;
    }

    pub fn spectate_aim(&self) -> Option<Aim> {
        self.spectate_aim
    }

    pub fn set_parallel_aim(&mut self, on: bool) {
        self.parallel_aim = on;
    }

    pub fn parallel_aim(&self) -> bool {
        self.parallel_aim
    }

    pub fn set_external_skins(&mut self, tab: Tab, skins: [Sym; 2]) {
        self.external_skins[tab.index()] = skins;
    }

    pub fn external_skin(&self, tab: Tab, unit: usize) -> Sym {
        self.external_skins[tab.index()][unit]
    }

    pub fn push_wave(&mut self, x: f32, y: f32) {
        self.waves.push(Wave { x, y, at: self.now });
    }

    pub fn waves(&self) -> &[Wave] {
        &self.waves
    }

    pub fn set_control(&mut self, control: Control) {
        self.control = control;
    }

    pub fn control(&self) -> Control {
        self.control
    }

    pub fn set_border(&mut self, border: Border) {
        self.border = border;
    }

    pub fn border(&self) -> Border {
        self.border
    }

    pub fn add_own(&mut self, tab: Tab, id: u32) {
        let own = &mut self.own[tab.index()];
        if !own.contains(&id) {
            own.push(id);
        }
    }

    pub fn remove_own(&mut self, tab: Tab, id: u32) {
        self.own[tab.index()].retain(|&o| o != id);
    }

    pub fn clear_own(&mut self, tab: Tab) {
        self.own[tab.index()].clear();
    }

    pub fn own(&self, tab: Tab) -> &[u32] {
        &self.own[tab.index()]
    }

    pub fn tab_dot(&self, tab: Tab) -> Option<MapDot> {
        let b = self.border;
        if !b.is_valid() {
            return None;
        }
        let own: Vec<&Cell> = self
            .own(tab)
            .iter()
            .filter_map(|&id| self.cell(id))
            .collect();
        let biggest = own.iter().max_by(|a, b| a.to.r.total_cmp(&b.to.r))?;
        let area: f32 = own.iter().map(|c| c.to.r * c.to.r).sum();
        let (sx, sy) = own.iter().fold((0.0, 0.0), |(sx, sy), c| {
            let a = c.to.r * c.to.r;
            (sx + c.to.x * a, sy + c.to.y * a)
        });
        let (w, h) = (b.max_x - b.min_x, b.max_y - b.min_y);
        Some(MapDot {
            x: (sx / area - b.min_x) / w,
            y: (sy / area - b.min_y) / h,
            r: area.sqrt() / w,
            color: biggest.color,
        })
    }

    pub fn set_server_view(&mut self, tab: Tab, view: Option<ServerView>) {
        self.server_views[tab.index()] = view;
    }

    pub fn server_view(&self) -> Option<ServerView> {
        self.server_views[self.control.tab.index()]
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cell(&self, id: u32) -> Option<&Cell> {
        self.slots.get(&id).map(|&slot| &self.cells[slot])
    }

    pub fn eaten(&self) -> &[Eaten] {
        &self.eaten
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn updated_at(&self) -> f64 {
        self.now
    }

    pub fn food_revision(&self) -> u64 {
        self.food_revision
    }

    pub fn food_moved_at(&self) -> Option<f64> {
        self.food_moved_at
    }
}
