//! Records what one player receives over protocol 6 and over the compact
//! protocol, from the same world, for the client's test that decodes both and
//! compares the scenes. Regenerate it after changing either encoding:
//!
//!   cargo run --release --example protocol_fixture
//!
//! The file is zlib-compressed. Inside, for protocol 6 and then compact: a u32
//! tick count, and per tick a u32 frame count followed by each frame as a u32
//! length and its bytes.

use multiogarii::core::config::Config;
use multiogarii::net::decode::ClientMsg;
use multiogarii::net::outbox::Outbox;
use multiogarii::world::data::Data;
use multiogarii::world::{ConnInfo, World};
use std::path::Path;
use std::sync::Arc;

const TICKS: u64 = 200;
const OUT: &str = "client/core/tests/fixtures/ogar6_and_compact.bin.z";

fn record(compact: bool) -> Vec<Vec<Vec<u8>>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cfg = Config {
        server_bots: 40,
        border_width: 4000.0,
        border_height: 4000.0,
        server_minion_threshold: 1000,
        ..Config::default()
    };
    let mut world = World::new(Arc::new(cfg), Data::load(&root.join("data")), 11, 0.0);
    let outbox = Arc::new(Outbox::new());
    let slot = world
        .connect(ConnInfo {
            ip: "10.0.0.1".into(),
            origin: None,
            user_agent: Some("x".repeat(60)),
            cache_control: Some("no-cache".into()),
            outbox: outbox.clone(),
        })
        .expect("connection accepted");
    world.client_message(slot, ClientMsg::Ready { protocol: 6, compact });
    world.player_mut(slot).spawn_size = 400.0;
    let mut ticks = Vec::new();
    for t in 1..=TICKS {
        if !world.player(slot).is_alive() && t.is_multiple_of(30) {
            world.client_message(slot, ClientMsg::Join("<doge>fixture".into()));
        }
        if t % 90 == 45 {
            world.client_message(slot, ClientMsg::SwitchUnit { tagged: true });
        }
        if t % 60 == 59 {
            world.client_message(slot, ClientMsg::Split);
        }
        let c = world.player(slot).center;
        let (dx, dy) = ((t % 7) as f64 * 300.0 - 900.0, (t % 5) as f64 * 300.0 - 600.0);
        let still = t % 100 < 15;
        let aim = if still { (c.x, c.y) } else { (c.x + dx, c.y + dy) };
        world.client_message(slot, ClientMsg::Mouse { x: aim.0, y: aim.1 });
        if t.is_multiple_of(4) {
            world.client_message(slot, ClientMsg::Eject);
        }
        world.tick(t as f64 * 40.0);
        ticks.push(outbox.drain().frames.iter().map(|f| f.to_vec()).collect());
    }
    ticks
}

fn main() {
    let mut out = Vec::new();
    for compact in [false, true] {
        let ticks = record(compact);
        out.extend_from_slice(&(ticks.len() as u32).to_le_bytes());
        for frames in ticks {
            out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
            for frame in frames {
                out.extend_from_slice(&(frame.len() as u32).to_le_bytes());
                out.extend_from_slice(&frame);
            }
        }
    }
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(&out, 9);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(OUT);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &packed).unwrap();
    println!("{} bytes ({} raw) written to {}", packed.len(), out.len(), path.display());
}
