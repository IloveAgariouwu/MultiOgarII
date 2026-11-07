use super::truncate_utf16;
use crate::net::packets::{self, Scramble};
use crate::world::World;
use rand::Rng;

const FIRST_PROTOCOL_WITH_CLEAR_ALL: u32 = 6;
const MOUSE_PACKET_MAX_COORD: f64 = 31767.0;
const SCRAMBLED_BORDER_MAX_EXTRA: f64 = 10065536.0;

fn without_controls(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

impl World {
    pub(in crate::world) fn scramble_player(&mut self, slot: u32) {
        let level = self.cfg.server_scramble_level;
        let scramble = if level == 0 {
            Scramble::default()
        } else {
            let max_x = (MOUSE_PACKET_MAX_COORD - self.border.width()).max(0.0);
            let max_y = (MOUSE_PACKET_MAX_COORD - self.border.height()).max(0.0);
            let x = max_x * self.rng.gen::<f64>();
            let y = max_y * self.rng.gen::<f64>();
            Scramble {
                id: self.rng.gen::<u32>(),
                x: if self.rng.gen_bool(0.5) { x } else { -x },
                y: if self.rng.gen_bool(0.5) { y } else { -y },
            }
        };
        self.player_mut(slot).scramble = scramble;
        self.views[slot as usize].border_counter = 0;
    }

    pub fn set_nickname(&mut self, slot: u32, text: &str) {
        let mut name = text;
        let mut skin: Option<String> = None;
        if let Some(rest) = text.strip_prefix('<') {
            if let Some(end) = rest.find('>') {
                let inner = &rest[..end];
                skin = Some(if inner == "r" { self.random_skin() } else { without_controls(inner) });
                name = &rest[end + 1..];
            }
        }
        let name = without_controls(name);
        let mut name = truncate_utf16(&name, self.cfg.player_max_nick_length).to_owned();
        if self.data.contains_bad_word(&name) {
            skin = None;
            name = "Hi there!".to_owned();
        }
        self.join_game(slot, &name, skin.as_deref());
    }

    fn random_skin(&mut self) -> String {
        if self.data.random_skins.is_empty() {
            return String::new();
        }
        let i = self.rng.gen_range(0..self.data.random_skins.len());
        self.data.random_skins[i].clone()
    }

    fn join_game(&mut self, slot: u32, name: &str, skin: Option<&str>) {
        let player = self.player_mut(slot);
        if let Some(skin) = skin.filter(|s| !s.is_empty()) {
            player.set_skin(skin);
        }
        player.set_name(name);
        player.spectate = false;
        player.free_roam = false;
        player.active = 0;
        player.pending_splits = [0; 2];
        if player.unit_packets {
            player.send(packets::active_unit(0));
        }

        if player.is_human() {
            if player.protocol < FIRST_PROTOCOL_WITH_CLEAR_ALL {
                self.send_removal_of_everything(slot);
            }
            let player = self.player(slot);
            player.send(packets::clear_all());
            self.views[slot as usize].client_nodes.clear();
            self.scramble_player(slot);
            let level = self.cfg.server_scramble_level;
            let b = self.border;
            if level < 2 {
                let player = self.player(slot);
                player.send(packets::set_border(
                    [b.min_x, b.min_y, b.max_x, b.max_y],
                    &player.scramble,
                    None,
                    player.protocol,
                ));
            } else if level == 3 {
                let extra = SCRAMBLED_BORDER_MAX_EXTRA * self.rng.gen::<f64>();
                let player = self.player(slot);
                player.send(packets::set_border(
                    [b.min_x - extra, b.min_y - extra, b.max_x + extra, b.max_y + extra],
                    &player.scramble,
                    None,
                    player.protocol,
                ));
            }
        }
        self.on_player_spawn(slot, 0);
    }

    fn send_removal_of_everything(&mut self, slot: u32) {
        let player = self.player(slot);
        let nodes = &self.views[slot as usize].client_nodes;
        let mut b = Vec::with_capacity(16 + nodes.len() * 4);
        packets::begin_update_nodes(&mut b, &player.scramble, &[]);
        packets::end_nodes(&mut b);
        packets::put_remove_count(&mut b, player.protocol, nodes.len());
        for &(id, _) in nodes {
            packets::put_removed(&mut b, &player.scramble, id);
        }
        player.send(b);
    }
}
