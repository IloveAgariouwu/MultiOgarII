use super::{Player, PlayerKind};
use crate::net::outbox::{Outbox, CLOSE_NORMAL};
use crate::world::cell::size_from_mass;
use crate::world::view::ViewState;
use crate::world::World;
use rand::Rng;
use std::sync::Arc;

impl World {
    pub(in crate::world) fn new_player(&mut self, kind: PlayerKind, conn: Option<Arc<Outbox>>) -> u32 {
        let slot = self.free_players.pop().unwrap_or_else(|| {
            self.players.push(None);
            self.views.push(ViewState::default());
            self.players.len() as u32 - 1
        });
        self.last_pid += 1;
        let mut player = Player::new(slot, self.last_pid, kind, conn);
        player.has_minions = self.cfg.server_minions > 0;
        player.last_alive = self.now;
        self.players[slot as usize] = Some(player);
        self.views[slot as usize].reset();
        self.order.push(slot);
        self.mode_on_player_init(slot);
        self.scramble_player(slot);
        slot
    }

    pub(in crate::world) fn free_player(&mut self, slot: u32) {
        self.players[slot as usize] = None;
        self.views[slot as usize].reset();
        self.free_players.push(slot);
    }

    pub fn add_bot(&mut self) -> u32 {
        let slot = self.new_player(PlayerKind::Bot, None);
        let name = if self.data.bot_names.is_empty() {
            let name = format!("Bot | {}", self.bot_count);
            self.bot_count += 1;
            name
        } else {
            let i = self.rng.gen_range(0..self.data.bot_names.len());
            self.data.bot_names[i].clone()
        };
        self.set_nickname(slot, &name);
        slot
    }

    pub fn add_minion(&mut self, owner: u32, name: &str, mass: Option<f64>) {
        let (min, max) = (self.cfg.minion_start_size, self.cfg.minion_max_start_size);
        let size = match mass {
            Some(mass) => size_from_mass(mass),
            None if max > min => (self.rng.gen::<f64>() * (max - min) + min).floor(),
            None => min,
        };
        let owner_pid = self.player(owner).pid;
        let slot = self.new_player(PlayerKind::Minion { owner_pid, owner }, None);
        self.player_mut(slot).spawn_size = size;
        let name = if name.is_empty() { self.cfg.default_name.clone() } else { name.to_owned() };
        self.set_nickname(slot, &name);
    }

    pub(in crate::world) fn check_connection(&mut self, slot: u32) {
        match self.player(slot).kind {
            PlayerKind::Human => {
                let player = self.player(slot);
                if !player.connected {
                    let wait = self.cfg.player_disconnect_time;
                    let elapsed = (self.now - player.close_time) / 1000.0;
                    if wait <= 0.0 || !player.is_alive() || elapsed >= wait {
                        self.retire(slot);
                    }
                    return;
                }
                let timeout = self.cfg.server_timeout;
                if !player.close_requested
                    && timeout > 0.0
                    && (self.now - player.last_alive) / 1000.0 >= timeout
                {
                    self.player_mut(slot).kick(CLOSE_NORMAL, "Connection timeout");
                }
            }
            PlayerKind::Bot => {
                if self.player(slot).close_requested {
                    self.retire(slot);
                } else if !self.player(slot).is_alive() {
                    self.on_player_spawn(slot, 0);
                }
            }
            PlayerKind::Minion { owner_pid, owner } => {
                let leader = self.players[owner as usize]
                    .as_ref()
                    .filter(|o| o.pid == owner_pid && o.connected && o.has_minions)
                    .map(|o| (o.minion_frozen, o.minion_split, o.minion_eject, o.mouse()));
                let closing = self.player(slot).close_requested;
                let Some((frozen, split, eject, mouse)) = leader.filter(|_| !closing) else {
                    self.retire(slot);
                    return;
                };
                if !self.player(slot).is_alive() {
                    self.on_player_spawn(slot, 0);
                }
                let player = self.player_mut(slot);
                player.frozen = frozen;
                player.split_requested = split;
                player.eject_requested = eject;
                player.units[0].mouse = mouse;
            }
        }
    }

    fn retire(&mut self, slot: u32) {
        self.remove_player_cells(slot);
        self.player_mut(slot).removed = true;
    }

    pub fn remove_player_cells(&mut self, slot: u32) {
        for unit in 0..2 {
            while let Some(&cell) = self.player(slot).units[unit].cells.first() {
                self.remove_cell(cell);
            }
        }
    }
}
