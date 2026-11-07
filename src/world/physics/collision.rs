use super::split::virus_splits;
use crate::core::math::Vec2;
use crate::net::outbox::CLOSE_NORMAL;
use crate::world::cell::{Kind, EAT_RATIO};
use crate::world::mode::ModeKind;
use crate::world::World;
use rand::Rng;
use std::f64::consts::PI;

const MIN_MERGE_AGE_TICKS: u64 = 13;
const EXPERIMENTAL_VIRUS_PUSH_DISTANCE: f64 = 220.0;
const NO_GROW_MIN_HUNTER_RADIUS: f64 = 250.0;
const NO_GROW_MAX_PREY_RADIUS: f64 = 41.0;

#[derive(Clone, Copy, Debug)]
pub struct Manifold {
    pub cell: u32,
    pub check: u32,
    pub d: f64,
    pub p: Vec2,
}

impl World {
    pub(in crate::world) fn manifold(&self, cell: u32, check: u32) -> Manifold {
        let p = self.cells[check].pos - self.cells[cell].pos;
        Manifold { cell, check, d: p.dist(), p }
    }

    pub(in crate::world) fn update_player_cells(&mut self) {
        let mut hits = std::mem::take(&mut self.scratch_hits);
        let mut eats = std::mem::take(&mut self.scratch_manifolds);
        eats.clear();

        let mut i = 0;
        while i < self.player_cells.len() {
            let slot = self.player_cells[i];
            i += 1;
            if !self.cells[slot].alive {
                continue;
            }
            hits.clear();
            self.grid.query_into(self.cells[slot].bound(), &mut hits);
            for &check in &hits {
                let m = self.manifold(slot, check);
                if self.check_rigid(&m) {
                    self.resolve_rigid(&m);
                } else if check != slot {
                    eats.push(m);
                }
            }
            let owner = self.cells[slot].owner;
            self.move_player(slot);
            self.boost_cell(slot);
            self.auto_split(slot);
            if self.player(owner).is_minion {
                self.player(owner).close(CLOSE_NORMAL, "Minion");
                self.remove_cell(slot);
            }
        }
        for m in &eats {
            self.resolve_collision(m);
        }

        self.scratch_hits = hits;
        self.scratch_manifolds = eats;
    }

    fn check_rigid(&self, m: &Manifold) -> bool {
        let (a, b) = (&self.cells[m.cell], &self.cells[m.check]);
        if !a.has_owner() || !b.has_owner() {
            return false;
        }
        if a.owner != b.owner || a.unit != b.unit {
            let (pa, pb) = (self.player(a.owner), self.player(b.owner));
            if (pa.is_minion_kind() || pb.is_minion_kind()) && self.cfg.minion_collide_team == 0 {
                return false;
            }
            return self.mode.have_teams() && pa.team == pb.team;
        }
        let age = if self.cfg.mobile_physics != 0 { 1 } else { MIN_MERGE_AGE_TICKS };
        if a.age(self.ticks) < age || b.age(self.ticks) < age {
            return false;
        }
        !a.can_remerge || !b.can_remerge
    }

    pub fn resolve_rigid(&mut self, m: &Manifold) {
        if m.d == 0.0 {
            return;
        }
        let (r1, r2_a) = (self.cells[m.cell].radius, self.cells[m.cell].radius2);
        let (r2, r2_b) = (self.cells[m.check].radius, self.cells[m.check].radius2);
        let push = (r1 + r2 - m.d) / m.d;
        if push <= 0.0 {
            return;
        }
        let sum = r2_a + r2_b;
        let (share_a, share_b) = (push * r2_a / sum, push * r2_b / sum);
        self.cells[m.cell].pos -= m.p * share_b;
        self.cells[m.check].pos += m.p * share_a;
    }

    pub fn resolve_collision(&mut self, m: &Manifold) {
        let (mut prey, mut hunter) = (m.cell, m.check);
        if !self.cells[prey].alive || !self.cells[hunter].alive {
            return;
        }
        if self.cells[prey].radius > self.cells[hunter].radius {
            std::mem::swap(&mut prey, &mut hunter);
        }
        let (p, h) = (&self.cells[prey], &self.cells[hunter]);
        let div = if self.cfg.mobile_physics != 0 { 20.0 } else { 3.0 };
        if m.d >= h.radius - p.radius / div {
            return;
        }
        if p.has_owner() && p.owner == h.owner && p.unit == h.unit {
            if p.age(self.ticks) < MIN_MERGE_AGE_TICKS || h.age(self.ticks) < MIN_MERGE_AGE_TICKS {
                return;
            }
        } else if h.radius < p.radius * EAT_RATIO || !self.can_eat(hunter, prey) {
            return;
        }
        self.on_eat(hunter, prey);
        self.on_eaten(prey, hunter);
        self.cells[prey].killer = self.cells[hunter].id;
        self.remove_cell(prey);
    }

    fn can_eat(&self, hunter: u32, prey: u32) -> bool {
        let h = &self.cells[hunter];
        let prey_kind = self.cells[prey].kind;
        match h.kind {
            Kind::Player => true,
            Kind::Virus if h.is_mother => {
                let max = self.cfg.mother_cell_max_mass;
                !(max != 0.0 && h.mass >= max)
                    && matches!(prey_kind, Kind::Player | Kind::Virus | Kind::Ejected)
            }
            Kind::Virus => {
                self.virus_list.len() < self.cfg.virus_max_amount && prey_kind == Kind::Ejected
            }
            _ => false,
        }
    }

    fn on_eat(&mut self, hunter: u32, prey: u32) {
        let h = &self.cells[hunter];
        if h.kind == Kind::Virus && !h.is_mother {
            let angle = self.cells[prey].boost_direction.angle();
            if self.mode.kind == ModeKind::Experimental {
                self.set_boost(hunter, EXPERIMENTAL_VIRUS_PUSH_DISTANCE, angle);
                return;
            }
            let grown = (self.cells[hunter].radius2 + self.cells[prey].radius2).sqrt();
            self.cells[hunter].set_size(grown);
            if grown >= self.cfg.virus_max_size {
                self.cells[hunter].set_size(self.cfg.virus_min_size);
                self.shoot_virus(hunter, angle);
            }
            self.update_grid(hunter);
            return;
        }
        let p = &self.cells[prey];
        let mut gain = p.radius2;
        if self.cfg.player_bot_grow == 0
            && self.cells[hunter].radius >= NO_GROW_MIN_HUNTER_RADIUS
            && p.radius <= NO_GROW_MAX_PREY_RADIUS
            && p.kind == Kind::Player
        {
            gain = 0.0;
        }
        let grown = (self.cells[hunter].radius2 + gain).sqrt();
        self.cells[hunter].set_size(grown);
        if self.cells[hunter].is_mother {
            self.update_grid(hunter);
        }
    }

    fn on_eaten(&mut self, prey: u32, hunter: u32) {
        if self.cells[prey].kind != Kind::Virus || !self.cells[hunter].has_owner() {
            return;
        }
        let (owner, unit) = (self.cells[hunter].owner, self.cells[hunter].unit);
        let max_cells = if self.cfg.virus_max_cells > 0 {
            self.cfg.virus_max_cells as usize
        } else {
            self.cfg.player_max_cells
        };
        let have = self.player(owner).units[unit as usize].cells.len();
        let splits = virus_splits(
            self.cells[hunter].mass,
            max_cells as i64 - have as i64,
            self.cfg.virus_max_popped_size,
            self.cfg.virus_equal_pop_size != 0,
        );
        for mass in splits {
            let angle = 2.0 * PI * self.rng.gen::<f64>();
            self.split_player_cell(owner, unit, hunter, angle, mass);
        }
    }
}
