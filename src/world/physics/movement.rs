use crate::core::math::Vec2;
use crate::world::cell::{Kind, NOT_LISTED};
use crate::world::World;

impl World {
    pub(in crate::world) fn move_boosted_nodes(&mut self) {
        let mut hits = std::mem::take(&mut self.scratch_hits);
        let rigid_ejected = self.cfg.mobile_physics == 0;
        let mut i = 0;
        while i < self.moving.len() {
            let slot = self.moving[i];
            i += 1;
            self.boost_cell(slot);
            hits.clear();
            self.grid.query_into(self.cells[slot].bound(), &mut hits);
            for &check in &hits {
                if !self.cells[check].alive {
                    continue;
                }
                let m = self.manifold(slot, check);
                if rigid_ejected
                    && self.cells[slot].kind == Kind::Ejected
                    && self.cells[check].kind == Kind::Ejected
                {
                    self.resolve_rigid(&m);
                    self.update_grid(check);
                } else {
                    self.resolve_collision(&m);
                }
            }
        }
        self.scratch_hits = hits;
    }

    pub(in crate::world) fn decay_player_cells(&mut self) {
        for i in 0..self.player_cells.len() {
            let slot = self.player_cells[i];
            self.update_size_decay(slot);
        }
    }

    pub fn update_size_decay(&mut self, slot: u32) {
        let mut rate = self.cfg.player_decay_rate;
        let cap = self.cfg.player_decay_cap;
        let min_size = self.cfg.player_min_size;
        let decay_mod = self.mode.decay_mod();
        let cell = &mut self.cells[slot];
        if rate == 0.0 || cell.radius <= min_size {
            return;
        }
        if cap != 0.0 && cell.mass > cap {
            rate *= 10.0;
        }
        let decay = 1.0 - rate * decay_mod;
        cell.set_size((cell.radius2 * decay).sqrt());
    }

    pub fn move_player(&mut self, slot: u32) {
        let (owner, unit) = (self.cells[slot].owner, self.cells[slot].unit as usize);
        let player = self.player(owner);
        if !player.connected || player.frozen {
            return;
        }
        let mouse = player.units[unit].mouse;
        let ticks = self.ticks;
        let speed_factor = self.cfg.player_speed;
        let recombine = self.cfg.player_recombine_time;

        let cell = &mut self.cells[slot];
        let d = mouse - cell.pos;
        let dist = d.dist();
        let mut step = 2.2 * cell.radius.powf(-0.45) * 40.0 * speed_factor;
        step = step.min(dist);
        if dist != 0.0 {
            step /= dist;
        }
        if step == 0.0 {
            return;
        }
        cell.pos += d * step;

        if recombine == 0.0 {
            cell.can_remerge = cell.boost_distance < 100.0;
        } else {
            let base = recombine.max(cell.radius * 0.2) * 25.0;
            cell.can_remerge = cell.age(ticks) as f64 >= base;
        }
    }

    pub fn boost_cell(&mut self, slot: u32) {
        let cell = &mut self.cells[slot];
        let speed = cell.boost_distance / 9.0;
        cell.boost_distance -= speed;
        let direction = cell.boost_direction;
        cell.pos += direction * speed;
        self.check_border(slot);
        self.update_grid(slot);
    }

    pub(in crate::world) fn stop_moving(&mut self, slot: u32) {
        let index = self.cells[slot].moving_index;
        if index == NOT_LISTED {
            return;
        }
        self.cells[slot].moving_index = NOT_LISTED;
        self.moving.swap_remove(index as usize);
        if let Some(&moved) = self.moving.get(index as usize) {
            self.cells[moved].moving_index = index;
        }
    }

    fn check_border(&mut self, slot: u32) {
        let b = self.border;
        let cell = &mut self.cells[slot];
        let r = cell.radius / 2.0;
        if cell.pos.x < b.min_x + r || cell.pos.x > b.max_x - r {
            cell.boost_direction.x *= -1.0;
            cell.pos.x = cell.pos.x.max(b.min_x + r).min(b.max_x - r);
        }
        if cell.pos.y < b.min_y + r || cell.pos.y > b.max_y - r {
            cell.boost_direction.y *= -1.0;
            cell.pos.y = cell.pos.y.max(b.min_y + r).min(b.max_y - r);
        }
    }

    pub fn set_boost(&mut self, slot: u32, distance: f64, angle: f64) {
        let cell = &mut self.cells[slot];
        cell.boost_distance = distance;
        cell.boost_direction = Vec2::from_angle(angle);
        cell.moving = true;
        if !cell.has_owner() && cell.moving_index == NOT_LISTED {
            cell.moving_index = self.moving.len() as u32;
            self.moving.push(slot);
        }
    }
}
