use super::cell::{CellStore, Kind, Rgb, NOT_LISTED, NO_OWNER};
use super::World;
use crate::core::math::{Rect, Vec2};
use crate::net::packets;
use rand::seq::SliceRandom;
use rand::Rng;

const VIRUS_COLOR: Rgb = Rgb::new(0x33, 0xFF, 0x33);
const SAFE_SPAWN_ATTEMPTS: u32 = 10;

impl World {
    pub(super) fn update_grid(&mut self, slot: u32) {
        let cell = &self.cells[slot];
        self.grid.update(slot, cell.pos.x, cell.pos.y, cell.radius);
    }

    pub fn random_color(&mut self) -> Rgb {
        let mut channels = [0xFF, 0x07, self.rng.gen::<u8>()];
        channels.shuffle(&mut self.rng);
        Rgb::new(channels[0], channels[1], channels[2])
    }

    pub fn random_pos(&mut self) -> Vec2 {
        let b = self.border;
        Vec2::new(
            b.min_x + b.width() * self.rng.gen::<f64>(),
            b.min_y + b.height() * self.rng.gen::<f64>(),
        )
    }

    fn touches_player(&self, pos: Vec2, radius: f64) -> bool {
        let cells = &self.cells;
        self.grid.find(Rect::around(pos, radius, radius), |slot| cells[slot].kind == Kind::Player)
    }

    pub(super) fn safe_position(&mut self, mut pos: Vec2, radius: f64) -> Vec2 {
        for _ in 0..SAFE_SPAWN_ATTEMPTS {
            if !self.touches_player(pos, radius) {
                break;
            }
            pos = self.random_pos();
        }
        pos
    }

    pub fn new_cell(&mut self, kind: Kind, pos: Vec2, radius: f64) -> u32 {
        let slot = self.cells.alloc(kind, pos, radius, self.ticks);
        self.grid.insert(slot, pos.x, pos.y, radius);
        match kind {
            Kind::Virus => {
                self.cells[slot].color = VIRUS_COLOR;
                self.cells[slot].list_index = self.virus_list.len() as u32;
                self.virus_list.push(slot);
            }
            Kind::Ejected => {
                self.cells[slot].list_index = self.ejected_list.len() as u32;
                self.ejected_list.push(slot);
            }
            Kind::Food => self.food_count += 1,
            Kind::Player => {}
        }
        slot
    }

    pub(super) fn new_player_cell(&mut self, owner: u32, unit: u8, pos: Vec2, radius: f64) -> u32 {
        let slot = self.cells.alloc(Kind::Player, pos, radius, self.ticks);
        self.grid.insert(slot, pos.x, pos.y, radius);
        let player = self.player_mut(owner);
        let u = &mut player.units[unit as usize];
        u.cells.push(slot);
        let color = u.color;
        let tagged = player.unit_packets;
        let announce = tagged || unit == player.active;
        let sc = player.scramble;
        let cell = &mut self.cells[slot];
        cell.owner = owner;
        cell.unit = unit;
        cell.color = color;
        self.player_cells.insert(0, slot);
        if announce {
            let id = self.cells[slot].id;
            let frame = if tagged {
                packets::add_unit_node(id, unit, &sc)
            } else {
                packets::add_node(id, &sc)
            };
            self.player(owner).send(frame);
        }
        slot
    }

    fn list_remove(list: &mut Vec<u32>, cells: &mut CellStore, slot: u32) {
        let index = cells[slot].list_index;
        if index == NOT_LISTED {
            return;
        }
        cells[slot].list_index = NOT_LISTED;
        list.swap_remove(index as usize);
        if let Some(&moved) = list.get(index as usize) {
            cells[moved].list_index = index;
        }
    }

    pub fn remove_cell(&mut self, slot: u32) {
        debug_assert!(self.cells[slot].alive, "double removal of slot {slot}");
        self.cells[slot].alive = false;
        self.grid.remove(slot);
        self.stop_moving(slot);
        self.graveyard.push(slot);
        match self.cells[slot].kind {
            Kind::Food => {
                self.food_count -= 1;
                if !self.cells[slot].no_respawn {
                    self.spawn_food();
                }
            }
            Kind::Ejected => Self::list_remove(&mut self.ejected_list, &mut self.cells, slot),
            Kind::Virus => {
                Self::list_remove(&mut self.virus_list, &mut self.cells, slot);
                if self.cells[slot].is_mother {
                    self.mode_on_mother_removed(slot);
                } else if self.virus_list.len() < self.cfg.virus_amount {
                    self.spawn_virus();
                }
            }
            Kind::Player => {
                let index = self.player_cells.iter().position(|&cell| cell == slot);
                self.player_cells.swap_remove(index.expect("a player cell is listed"));
                self.detach_from_owner(slot);
            }
        }
    }

    pub(super) fn unlist_virus(&mut self, slot: u32) {
        Self::list_remove(&mut self.virus_list, &mut self.cells, slot);
    }

    fn detach_from_owner(&mut self, slot: u32) {
        let (owner, unit) = (self.cells[slot].owner, self.cells[slot].unit);
        debug_assert!(owner != NO_OWNER);
        let player = self.player_mut(owner);
        let cells = &mut player.units[unit as usize].cells;
        let index = cells.iter().position(|&c| c == slot).expect("a cell is listed by its owner");
        cells.swap_remove(index);
        if !cells.is_empty() || unit != player.active {
            return;
        }
        let other = unit ^ 1;
        if player.units[other as usize].cells.is_empty() {
            player.active = 0;
            if player.unit_packets {
                player.send(packets::active_unit(0));
            }
        } else {
            player.active = other;
            self.announce_active_unit(owner);
        }
    }

    fn spawn_food(&mut self) {
        let pos = self.random_pos();
        let mut size = self.cfg.food_min_size;
        if self.cfg.food_mass_grow != 0 {
            size += (self.cfg.food_max_size - size) * self.rng.gen::<f64>();
        }
        let color = self.random_color();
        let slot = self.new_cell(Kind::Food, pos, size);
        self.cells[slot].color = color;
    }

    pub fn spawn_virus(&mut self) {
        let size = self.cfg.virus_min_size;
        let wanted = self.random_pos();
        let pos = self.safe_position(wanted, size);
        self.new_cell(Kind::Virus, pos, size);
    }

    pub(super) fn spawn_cells(&mut self) {
        for _ in 0..self.cfg.food_amount {
            self.spawn_food();
        }
        for _ in 0..self.cfg.virus_amount {
            self.spawn_virus();
        }
    }

    pub fn spawn_player(&mut self, slot: u32, unit: u8, mut pos: Vec2) {
        let mut size = self.cfg.player_start_size;
        if self.player(slot).spawn_size != 0.0 {
            size = self.player(slot).spawn_size;
        }

        if !self.ejected_list.is_empty() {
            let index = self.rng.gen_range(0..self.ejected_list.len());
            let eject = &self.cells[self.ejected_list[index]];
            let (eject_pos, eject_color, eject_radius) = (eject.pos, eject.color, eject.radius);
            let resting = eject.boost_distance < 1.0;
            if resting && self.rng.gen::<f64>() <= self.cfg.eject_spawn_percent {
                pos = eject_pos;
                self.player_mut(slot).units[unit as usize].color = eject_color;
                size = size.max(eject_radius * 1.15);
            }
        }

        if !self.player(slot).is_minion_kind() {
            pos = self.safe_position(pos, size);
        }
        self.new_player_cell(slot, unit, pos, size);
        self.player_mut(slot).units[unit as usize].mouse = pos;
    }
}
