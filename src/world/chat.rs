use super::cell::Rgb;
use super::player::{self, ROLE_ADMIN, ROLE_MODER};
use super::World;
use crate::net::packets;
use bytes::Bytes;

const MAX_CHAT_UTF16: usize = 64;
const CHAT_COLOR: Rgb = Rgb::new(0x9B, 0x9B, 0x9B);

impl World {
    pub(in crate::world) fn on_chat(&mut self, from: u32, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        let text = text.trim();
        if text.is_empty() || self.cfg.server_chat == 0 {
            return;
        }
        if let Some(command) = text.strip_prefix('/') {
            self.player_command(from, command);
            return;
        }
        let text = player::truncate_utf16(text, MAX_CHAT_UTF16);
        if self.cfg.server_chat_ascii != 0 && text.chars().any(|c| !(' '..='\x7F').contains(&c)) {
            self.tell(from, "Message failed - You can use ASCII text only!");
            return;
        }
        if self.cfg.bad_word_filter == 1 && self.data.contains_bad_word(text) {
            let refusal = "Message failed - Stop insulting others! Keep calm and be friendly please.";
            self.tell(from, refusal);
            return;
        }
        self.send_chat(Some(from), None, text);
    }

    pub fn tell(&self, slot: u32, text: &str) {
        self.send_chat(None, Some(slot), text);
    }

    pub fn broadcast(&self, text: &str) {
        self.send_chat(None, None, text);
    }

    fn send_chat(&self, from: Option<u32>, to: Option<u32>, text: &str) {
        let mut name = "SERVER";
        let mut color = CHAT_COLOR;
        let mut flags = 0x80;
        let mut team = None;
        if let Some(sender) = from {
            let p = self.player(sender);
            flags = match p.role {
                ROLE_ADMIN => 0x40,
                ROLE_MODER => 0x20,
                _ => 0,
            };
            name = if !p.name.is_empty() {
                &p.name
            } else if p.is_alive() {
                "An unnamed cell"
            } else {
                "Spectator"
            };
            if let Some(&first) = p.active_unit().cells.first() {
                color = self.cells[first].color;
            }
            if self.cfg.separate_chat_for_teams != 0 && self.mode.have_teams() {
                team = Some(p.team);
            }
        }
        let mut encoded: [Option<Bytes>; 2] = [None, None];
        let targets: &[u32] = match &to {
            Some(slot) => std::slice::from_ref(slot),
            None => &self.order,
        };
        for &slot in targets {
            let player = self.player(slot);
            if !player.is_human() || team.is_some_and(|t| t != player.team) {
                continue;
            }
            let frame = encoded[packets::uses_ucs2(player.protocol) as usize]
                .get_or_insert_with(|| {
                    packets::chat(flags, color, name, text, player.protocol).into()
                });
            player.send(frame.clone());
        }
    }
}
