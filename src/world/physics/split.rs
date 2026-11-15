use crate::core::math::Vec2;
use crate::world::cell::{size_from_mass, Kind};
use crate::world::mode::MOTHER_SIZE;
use crate::world::World;
use rand::Rng;
use std::f64::consts::PI;

impl World {
    pub(in crate::world) fn auto_split(&mut self, slot: u32) {
        let (owner, unit) = (self.cells[slot].owner, self.cells[slot].unit);
        let player = self.player(owner);
        let max_size = self.cfg.player_max_size;
        let cells = player.units[unit as usize].cells.len();
        if self.cells[slot].radius < max_size {
            return;
        }
        if cells >= self.cfg.player_max_cells || self.cfg.mobile_physics != 0 {
            self.cells[slot].set_size(max_size);
        } else {
            let angle = self.rng.gen::<f64>() * 2.0 * PI;
            let mass = self.cells[slot].mass * 0.5;
            self.split_player_cell(owner, unit, slot, angle, mass);
        }
    }

    pub fn split_player_cell(&mut self, owner: u32, unit: u8, parent: u32, angle: f64, mass: f64) {
        let size = size_from_mass(mass);
        let parent_size = (self.cells[parent].radius2 - size * size).sqrt();
        if parent_size.is_nan() || parent_size <= 0.0 || parent_size < self.cfg.player_min_size {
            return;
        }
        self.cells[parent].set_size(parent_size);
        let pos = self.cells[parent].pos;
        let cell = self.new_player_cell(owner, unit, pos, size);
        self.set_boost(cell, self.cfg.split_velocity * size.powf(0.0122), angle);
    }

    pub fn split_cells(&mut self, owner: u32, unit: u8) {
        let max = self.cfg.player_max_cells;
        let min_size = self.cfg.player_min_split_size;
        let mouse = self.player(owner).units[unit as usize].mouse;
        let snapshot = self.player(owner).units[unit as usize].cells.clone();
        for slot in snapshot {
            let cell = &self.cells[slot];
            if cell.radius < min_size {
                continue;
            }
            if self.player(owner).units[unit as usize].cells.len() >= max {
                continue;
            }
            let mut d = mouse - cell.pos;
            if d.dist_sq() < 1.0 {
                d = Vec2::new(1.0, 0.0);
            }
            let mass = cell.mass * 0.5;
            self.split_player_cell(owner, unit, slot, d.angle(), mass);
        }
    }

    pub fn eject_mass(&mut self, owner: u32, unit: u8) {
        let (ticks, cooldown) = (self.ticks, self.cfg.eject_cooldown);
        let player = self.player_mut(owner);
        if player.last_eject.is_some_and(|last| ticks.saturating_sub(last) < cooldown) {
            return;
        }
        player.last_eject = Some(ticks);
        if player.frozen {
            return;
        }
        let min_eject = self.cfg.player_min_eject_size;
        let min_size = self.cfg.player_min_size;
        let loss = self.cfg.eject_size_loss;
        let mouse = self.player(owner).units[unit as usize].mouse;
        let snapshot = self.player(owner).units[unit as usize].cells.clone();
        for slot in snapshot {
            let cell = &mut self.cells[slot];
            if cell.radius < min_eject {
                continue;
            }
            let remaining = cell.radius2 - loss * loss;
            if remaining < min_size * min_size {
                continue;
            }
            cell.set_size(remaining.sqrt());

            let mut d = mouse - cell.pos;
            let dist = d.dist();
            d = if dist > 1.0 { d * (1.0 / dist) } else { Vec2::new(1.0, 0.0) };
            let pos = cell.pos + d * cell.radius;
            let color = cell.color;
            let angle = d.angle() + self.rng.gen::<f64>() * 0.6 - 0.3;
            let kind = if self.cfg.eject_virus != 0 { Kind::Virus } else { Kind::Ejected };
            let ejected = self.new_cell(kind, pos, self.cfg.eject_size);
            self.cells[ejected].color = color;
            self.set_boost(ejected, self.cfg.eject_velocity, angle);
        }
    }

    pub fn shoot_virus(&mut self, parent: u32, angle: f64) {
        let pos = self.cells[parent].pos;
        let virus = self.new_cell(Kind::Virus, pos, self.cfg.virus_min_size);
        self.set_boost(virus, self.cfg.virus_velocity, angle);
    }

    pub(in crate::world) fn update_mother_cell(&mut self, slot: u32) {
        if self.cells[slot].radius == MOTHER_SIZE {
            return;
        }
        let food_size = self.cfg.food_min_size;
        let mut size = self.cells[slot].radius;
        for _ in 0..2 {
            size = (size * size - food_size * food_size * 2.0).sqrt().max(MOTHER_SIZE);
            self.cells[slot].set_size(size);
            let angle = self.rng.gen::<f64>() * 2.0 * PI;
            let pos = self.cells[slot].pos + Vec2::from_angle(angle) * size;
            let food = self.new_cell(Kind::Food, pos, food_size);
            self.cells[food].color = self.random_color();
            self.cells[food].no_respawn = true;
            let distance = 32.0 + 42.0 * self.rng.gen::<f64>();
            self.set_boost(food, distance, angle);
        }
        self.update_grid(slot);
    }
}

pub fn virus_splits(mass: f64, mut cells_left: i64, max_popped: f64, equal: bool) -> Vec<f64> {
    let mut splits = Vec::new();
    if cells_left <= 0 {
        return splits;
    }
    let split_min = max_popped * max_popped / 100.0;
    if equal {
        let count = ((mass / split_min) as i64).min(cells_left);
        let part = mass / (1 + count) as f64;
        splits.resize(count as usize, part);
        return splits;
    }
    if mass / (cells_left as f64) < split_min {
        let mut count = 2i64;
        let mut part = mass / count as f64;
        while part > split_min && count * 2 < cells_left {
            count *= 2;
            part = mass / count as f64;
        }
        part = mass / (count + 1) as f64;
        splits.resize(count as usize, part);
        return splits;
    }
    let mut part = mass / 2.0;
    let mut left = mass / 2.0;
    while cells_left > 0 {
        cells_left -= 1;
        if left / (cells_left as f64) < split_min {
            part = left / cells_left as f64;
            for _ in 0..cells_left {
                splits.push(part);
            }
            cells_left = 0;
        }
        while part >= left && cells_left > 0 {
            part /= 2.0;
        }
        splits.push(part);
        left -= part;
    }
    splits
}
