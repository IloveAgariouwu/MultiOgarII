//! Times `World::tick` with only bots in the world, from a fixed seed, so two
//! builds can be compared without network or scheduling noise.
//!
//!   cargo run --release --example tick -- [bots] [ticks] [seed]
//!
//! Prints the median, p95 and mean of the per-tick time after a warm-up.

use multiogarii::core::config::Config;
use multiogarii::world::data::Data;
use multiogarii::world::World;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const WARMUP_TICKS: usize = 250;
const TICK_MS: f64 = 40.0;

fn main() {
    let mut args = std::env::args().skip(1).map(|a| a.parse::<u64>().expect("numeric argument"));
    let bots = args.next().unwrap_or(300) as usize;
    let ticks = args.next().unwrap_or(1500) as usize;
    let seed = args.next().unwrap_or(1);

    let cfg = Config { server_bots: bots, ..Config::default() };
    let data = Data::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("data").as_path());
    let mut world = World::new(Arc::new(cfg), data, seed, 0.0);

    let mut now = 0.0;
    for _ in 0..WARMUP_TICKS {
        now += TICK_MS;
        world.tick(now);
    }
    let mut times = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        now += TICK_MS;
        let started = Instant::now();
        world.tick(now);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(f64::total_cmp);
    let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "bots {bots}  ticks {ticks}  median {:.3} ms  p95 {:.3} ms  mean {:.3} ms",
        at(0.5),
        at(0.95),
        mean
    );
}
