use super::{Input, PlayerKind};
use crate::core::math::Vec2;
use crate::net::decode::ClientMsg;
use crate::net::outbox::{Outbox, CLOSE_NORMAL};
use crate::net::packets;
use crate::world::{World, TICKS_PER_SECOND, VERSION};
use std::sync::Arc;

const CHAT_COOLDOWN_TICKS: u64 = 50;

pub(in crate::world) struct MinionSuspect {
    pub(in crate::world) slot: u32,
    pub(in crate::world) pid: u32,
    pub(in crate::world) connected_at: f64,
}

pub struct ConnInfo {
    pub ip: String,
    pub origin: Option<String>,
    pub user_agent: Option<String>,
    pub cache_control: Option<String>,
    pub outbox: Arc<Outbox>,
}

impl World {
    pub fn connect(&mut self, info: ConnInfo) -> Option<u32> {
        let refuse = |reason: &str| {
            info.outbox.close(CLOSE_NORMAL, reason);
            None
        };
        let max = self.cfg.server_max_connections;
        let humans = self.order.iter().map(|&s| self.player(s)).filter(|p| p.is_human() && p.connected);
        if max != 0 && humans.clone().count() >= max {
            return refuse("No slots");
        }
        if self.data.is_ip_banned(&info.ip) {
            return refuse("IP banned");
        }
        let limit = self.cfg.server_ip_limit;
        if limit != 0 && humans.filter(|p| p.ip == info.ip).count() >= limit {
            return refuse("IP limit reached");
        }
        if !self.origin_allowed(info.origin.as_deref()) {
            return refuse("Client not allowed");
        }

        let slot = self.new_player(PlayerKind::Human, Some(info.outbox));
        let player = self.player_mut(slot);
        player.ip = info.ip;
        let headers_ok = info.cache_control.as_ref().is_some_and(|cc| !cc.is_empty())
            && info.user_agent.as_ref().is_some_and(|ua| ua.len() >= 50);
        self.check_minion(slot, headers_ok);
        Some(slot)
    }

    fn origin_allowed(&self, origin: Option<&str>) -> bool {
        let bind = &self.cfg.client_bind;
        if bind.is_empty() {
            return true;
        }
        let Some(origin) = origin else { return false };
        let host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
        let host = match host.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or(v6),
            None => host.split(['/', ':']).next().unwrap_or(host),
        };
        bind.split(" - ")
            .map(str::trim)
            .filter(|allowed| !allowed.is_empty())
            .any(|allowed| {
                origin.eq_ignore_ascii_case(allowed)
                    || host.eq_ignore_ascii_case(allowed.trim_start_matches('[').trim_end_matches(']'))
            })
    }

    fn check_minion(&mut self, slot: u32, headers_ok: bool) {
        if !headers_ok {
            self.player_mut(slot).is_minion = true;
        }
        let threshold = self.cfg.server_minion_threshold;
        let uptime = (self.now - self.start_time) / 1000.0;
        if threshold != 0 && uptime >= self.cfg.server_minion_ignore_time {
            if self.minion_test.len() >= threshold {
                self.player_mut(slot).is_minion = true;
                for suspect in &self.minion_test {
                    if let Some(player) = &mut self.players[suspect.slot as usize] {
                        if player.connected && player.pid == suspect.pid {
                            player.is_minion = true;
                        }
                    }
                }
                self.minion_test.remove(0);
            }
            let pid = self.player(slot).pid;
            self.minion_test.push(MinionSuspect { slot, pid, connected_at: self.now });
        }
        if !self.player(slot).is_minion {
            for _ in 0..self.cfg.server_minions {
                self.add_minion(slot, "", None);
            }
        }
    }

    pub fn disconnect(&mut self, slot: u32) {
        let pid = self.player(slot).pid;
        self.minion_test.retain(|suspect| suspect.pid != pid);
        let now = self.now;
        let player = self.player_mut(slot);
        player.connected = false;
        player.close_time = now;
        player.split_requested = false;
        player.eject_requested = false;
        player.toggle_spectate = false;
        player.inputs.clear();
        player.pending_splits = [0; 2];
    }

    pub fn client_message(&mut self, slot: u32, msg: ClientMsg) {
        let (now, ticks) = (self.now, self.ticks);
        let player = self.player_mut(slot);
        player.last_alive = now;
        match msg {
            ClientMsg::Ready { protocol } => self.on_ready(slot, protocol),
            ClientMsg::Join(text) => {
                let dt = ticks.saturating_sub(player.last_join_tick);
                player.last_join_tick = ticks;
                if dt >= TICKS_PER_SECOND && !player.is_alive() {
                    self.set_nickname(slot, &text);
                }
            }
            ClientMsg::Spectate => {
                if !player.is_alive() {
                    player.spectate = true;
                    player.free_roam = false;
                }
            }
            ClientMsg::Mouse { x, y } => player.queue_input(Input::Mouse(Vec2::new(x, y))),
            ClientMsg::Split => player.queue_input(Input::Split),
            ClientMsg::SpectateToggle => player.toggle_spectate = true,
            ClientMsg::Eject => player.queue_input(Input::Eject),
            ClientMsg::MinionSplit => player.minion_split = true,
            ClientMsg::MinionEject => player.minion_eject = true,
            ClientMsg::MinionFreezeToggle => player.minion_frozen = !player.minion_frozen,
            ClientMsg::SwitchUnit { tagged } => {
                if tagged && !player.unit_packets {
                    player.unit_packets = true;
                    self.announce_units(slot);
                }
                self.player_mut(slot).queue_input(Input::SwitchUnit);
            }
            ClientMsg::Chat(text) => {
                let dt = ticks.saturating_sub(player.last_chat_tick);
                player.last_chat_tick = ticks;
                if dt >= CHAT_COOLDOWN_TICKS {
                    self.on_chat(slot, &text);
                }
            }
            ClientMsg::Stat => {}
        }
    }

    fn on_ready(&mut self, slot: u32, protocol: u32) {
        let version = format!("MultiOgarII {VERSION}");
        let b = self.border;
        let game = self.cfg.server_gamemode as u32;
        let player = self.player_mut(slot);
        player.protocol = protocol;
        player.send(packets::clear_all());
        player.send(packets::set_border(
            [b.min_x, b.min_y, b.max_x, b.max_y],
            &player.scramble,
            Some((game, &version)),
            protocol,
        ));
        self.tell(slot, &version);
        for welcome in [&self.cfg.server_welcome1, &self.cfg.server_welcome2] {
            if !welcome.is_empty() {
                self.tell(slot, welcome);
            }
        }
        if self.cfg.server_chat == 0 {
            self.tell(slot, "This server's chat is disabled.");
        }
        if protocol < 4 {
            let warning = format!("WARNING: Protocol {protocol} assumed as 4!");
            self.tell(slot, &warning);
        }
    }
}
