use std::collections::HashMap;
use std::sync::Arc;

use js_sys::Uint8Array;
use multiogarii::core::config::Config;
use multiogarii::net::decode::{ClientMsg, Decoded, Decoder};
use multiogarii::net::outbox::{Drained, Outbox};
use multiogarii::net::packets::server_stat;
use multiogarii::world::data::Data;
use multiogarii::world::{ConnInfo, World};
use wasm_bindgen::prelude::*;

const BOTS: usize = 20;

struct Conn {
    slot: Option<u32>,
    outbox: Arc<Outbox>,
    decoder: Decoder,
    closed: Option<String>,
}

#[wasm_bindgen]
pub struct Solo {
    world: World,
    conns: HashMap<u32, Conn>,
    last_id: u32,
}

#[wasm_bindgen]
impl Solo {
    /// `gamemode` is `serverGamemode`: 0 FFA, 1 Teams, 2 Experimental, 3 Rainbow.
    #[wasm_bindgen(constructor)]
    pub fn new(now: f64, gamemode: i32) -> Solo {
        let cfg = Config {
            server_bots: BOTS,
            server_gamemode: gamemode,
            ..Config::default()
        };
        Solo {
            world: World::new(Arc::new(cfg), Data::default(), now.to_bits(), now),
            conns: HashMap::new(),
            last_id: 0,
        }
    }

    pub fn open(&mut self, user_agent: String, now: f64) -> u32 {
        self.world.now = now;
        let outbox = Arc::new(Outbox::new());
        let slot = self.world.connect(ConnInfo {
            ip: "127.0.0.1".to_owned(),
            origin: None,
            user_agent: Some(user_agent),
            cache_control: Some("no-cache".to_owned()),
            outbox: outbox.clone(),
        });
        self.last_id += 1;
        self.conns.insert(
            self.last_id,
            Conn {
                slot,
                outbox,
                decoder: Decoder::default(),
                closed: None,
            },
        );
        self.last_id
    }

    pub fn send(&mut self, id: u32, data: &[u8], now: f64) {
        let Some(conn) = self.conns.get_mut(&id) else {
            return;
        };
        if conn.outbox.is_closing() {
            return;
        }
        match conn.decoder.decode(data) {
            Decoded::Msg(msg) => {
                if matches!(msg, ClientMsg::Stat) {
                    conn.outbox
                        .push(server_stat(&self.world.client_stats_json()));
                }
                if let Some(slot) = conn.slot {
                    self.world.now = now;
                    self.world.client_message(slot, msg);
                }
            }
            Decoded::Handled | Decoded::Ignored => {}
            Decoded::Close(code, reason) => conn.outbox.close(code, &reason),
        }
    }

    pub fn tick(&mut self, now: f64) {
        self.world.tick(now);
    }

    pub fn drain(&mut self, id: u32) -> Vec<Uint8Array> {
        let Some(conn) = self.conns.get_mut(&id) else {
            return Vec::new();
        };
        let Drained { frames, close } = conn.outbox.drain();
        if let Some((_, reason)) = close {
            conn.closed = Some(reason);
        }
        frames.iter().map(|f| Uint8Array::from(&f[..])).collect()
    }

    pub fn closed(&self, id: u32) -> Option<String> {
        self.conns.get(&id)?.closed.clone()
    }

    pub fn close(&mut self, id: u32) {
        if let Some(Conn {
            slot: Some(slot), ..
        }) = self.conns.remove(&id)
        {
            self.world.disconnect(slot);
        }
    }
}
