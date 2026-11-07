use super::Input;
use crate::core::math::Vec2;
use crate::net::packets;
use crate::world::World;

const KEYPRESS_COOLDOWN_TICKS: u64 = 40;

impl World {
    pub fn switch_unit(&mut self, slot: u32) {
        if self.cfg.server_multibox == 0 {
            return;
        }
        let player = self.player(slot);
        if !player.is_human() || player.spectate || !player.is_alive() {
            return;
        }
        let target = player.active ^ 1;
        let player = self.player_mut(slot);
        player.active = target;
        let spawn = player.units[target as usize].cells.is_empty();
        self.announce_active_unit(slot);
        if spawn {
            self.on_player_spawn(slot, target);
        }
    }

    /// Tells the client which unit is active. Legacy clients only know the
    /// cells of the active unit, so they get them again.
    pub(in crate::world) fn announce_active_unit(&self, slot: u32) {
        let player = self.player(slot);
        if player.unit_packets {
            player.send(packets::active_unit(player.active));
            return;
        }
        player.send(packets::clear_owned());
        for &cell in &player.active_unit().cells {
            player.send(packets::add_node(self.cells[cell].id, &player.scramble));
        }
    }

    /// Sends every owned cell with its unit, when the client starts using unit packets.
    pub(in crate::world) fn announce_units(&self, slot: u32) {
        let player = self.player(slot);
        player.send(packets::clear_owned());
        for (unit, u) in player.units.iter().enumerate() {
            for &cell in &u.cells {
                player.send(packets::add_unit_node(self.cells[cell].id, unit as u8, &player.scramble));
            }
        }
        player.send(packets::active_unit(player.active));
    }

    fn player_split(&mut self, slot: u32, unit: u8) {
        let ticks = self.ticks;
        let running = self.running;
        let player = self.player_mut(slot);
        if player.spectate {
            if ticks.saturating_sub(player.last_keypress_tick) >= KEYPRESS_COOLDOWN_TICKS {
                player.last_keypress_tick = ticks;
            }
        } else if running {
            if player.frozen {
                return;
            }
            self.split_cells(slot, unit);
        }
    }

    fn player_spectate_toggle(&mut self, slot: u32) {
        let ticks = self.ticks;
        let player = self.player_mut(slot);
        if player.spectate && ticks.saturating_sub(player.last_keypress_tick) >= KEYPRESS_COOLDOWN_TICKS {
            player.last_keypress_tick = ticks;
            player.free_roam = !player.free_roam;
        }
    }

    pub(in crate::world) fn process_input(&mut self, slot: u32) {
        let max_splits = self.cfg.player_max_cells;
        let mut eject = [false; 2];
        let inputs = std::mem::take(&mut self.player_mut(slot).inputs);
        for input in &inputs {
            let player = self.player_mut(slot);
            let active = player.active as usize;
            match *input {
                Input::Mouse(mouse) => {
                    let scramble = player.scramble;
                    player.set_mouse(Vec2::new(mouse.x - scramble.x, mouse.y - scramble.y));
                }
                Input::Split => {
                    let pending = &mut player.pending_splits[active];
                    *pending = (*pending + 1).min(max_splits);
                }
                Input::Eject => eject[active] = true,
                Input::SwitchUnit => self.switch_unit(slot),
            }
        }
        let player = self.player_mut(slot);
        // Keep the allocation for the next tick's inputs.
        player.inputs = inputs;
        player.inputs.clear();
        let active = player.active as usize;
        if std::mem::take(&mut player.split_requested) {
            player.pending_splits[active] = (player.pending_splits[active] + 1).min(max_splits);
        }
        eject[active] |= std::mem::take(&mut player.eject_requested);
        for unit in 0..2u8 {
            let player = self.player(slot);
            if eject[unit as usize] && !player.spectate && self.running {
                self.eject_mass(slot, unit);
            }
        }
        // One split per unit per tick; the rest wait for the next ticks.
        for unit in 0..2u8 {
            let pending = &mut self.player_mut(slot).pending_splits[unit as usize];
            if *pending > 0 {
                *pending -= 1;
                self.player_split(slot, unit);
            }
        }
        let player = self.player_mut(slot);
        if std::mem::take(&mut player.toggle_spectate) {
            self.player_spectate_toggle(slot);
        }
        let player = self.player_mut(slot);
        player.minion_split = false;
        player.minion_eject = false;
    }
}
