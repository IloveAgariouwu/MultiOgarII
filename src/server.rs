use crate::core::config::Config;
use crate::core::logger;
use crate::net::listener::accept_loop;
use crate::net::packets::server_stat;
use crate::net::socket::Event;
use crate::net::stats::stats_loop;
use crate::net::tls::load_tls;
use crate::net::tracker::ping_tracker;
use crate::world::data::Data;
use crate::world::{World, VERSION};
use std::io::BufRead;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use bytes::Bytes;
use tokio::net::TcpListener;

static CLOCK: OnceLock<(Instant, f64)> = OnceLock::new();

const TIME_STEP: Duration = Duration::from_millis(40);
const MAX_LAG: Duration = Duration::from_millis(120);
fn now_ms() -> f64 {
    let (boot, wall) = CLOCK.get_or_init(|| {
        let wall = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        (Instant::now(), wall.as_secs_f64() * 1000.0)
    });
    wall + boot.elapsed().as_secs_f64() * 1000.0
}

struct GameLoop {
    world: World,
    events: Receiver<Event>,
    console: Receiver<String>,
    conns: HashMap<u64, u32>,
    stats: Arc<Mutex<String>>,
    client_stat: Arc<Mutex<Bytes>>,
    stats_interval: Duration,
    tracker: tokio::runtime::Handle,
}

impl GameLoop {
    fn handle_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Open { id, info } => {
                    if let Some(slot) = self.world.connect(info) {
                        self.conns.insert(id, slot);
                    }
                }
                Event::Message { id, msg } => {
                    if let Some(&slot) = self.conns.get(&id) {
                        self.world.client_message(slot, msg);
                    }
                }
                Event::Closed { id } => {
                    if let Some(slot) = self.conns.remove(&id) {
                        self.world.disconnect(slot);
                    }
                }
            }
        }
        while let Ok(line) = self.console.try_recv() {
            self.world.console_command(&line);
        }
    }

    fn run(mut self) -> i32 {
        let mut next_stats = Instant::now();
        let mut due = Instant::now();
        loop {
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            let now = Instant::now();
            if now - due > MAX_LAG {
                due = now;
            }
            due += TIME_STEP;

            let ms = now_ms();
            self.world.now = ms;
            self.handle_events();
            let started = Instant::now();
            self.world.tick(ms);
            self.world.record_update_time(started.elapsed().as_secs_f64() * 1000.0);

            if let Some(code) = self.world.exit_requested {
                return code;
            }
            *self.client_stat.lock().unwrap() = server_stat(&self.world.client_stats_json()).into();
            if now >= next_stats {
                next_stats = now + self.stats_interval;
                *self.stats.lock().unwrap() = self.world.stats_json();
            }
            if let Some(body) = self.world.tracker_ping() {
                self.tracker.spawn(ping_tracker(body));
            }
        }
    }
}

fn worker_threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (cores / 2).clamp(1, 4)
}

pub struct Running {
    pub port: u16,
    pub stats_port: Option<u16>,
    pub game: std::thread::JoinHandle<i32>,
    pub console: Sender<String>,
    pub runtime: tokio::runtime::Runtime,
}

pub fn start(config: Config, data: Data) -> Result<Running, String> {
    let cfg = Arc::new(config);
    let threads = worker_threads();
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .enable_all()
        .thread_name("net")
        .build()
        .map_err(|e| e.to_string())?;

    let listener = runtime
        .block_on(TcpListener::bind((cfg.server_bind.as_str(), cfg.server_port)))
        .map_err(|e| {
            let hint = if e.kind() == std::io::ErrorKind::PermissionDenied {
                "Please make sure you are running MultiOgarII with root privileges."
            } else {
                "Please close out of Skype or change 'serverPort' in the config to a different port."
            };
            format!("Server could not bind to port {}!\n{hint}", cfg.server_port)
        })?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let (event_tx, event_rx) = mpsc::channel();
    let (console_tx, console_rx) = mpsc::channel();
    let world = World::new(cfg.clone(), data, rand::random(), now_ms());
    let acceptor = load_tls();
    logger::info(&format!("Running MultiOgarII {VERSION}, a FOSS agar.io server implementation."));
    logger::info(&format!("Game server started, on port {port}"));
    logger::info(&format!("Current game mode is {}", world.mode.name()));
    if cfg.server_bots > 0 {
        logger::info(&format!("Added {} player bots", cfg.server_bots));
    }

    let stats = Arc::new(Mutex::new(world.stats_json()));
    let mut stats_port = None;
    if cfg.server_stats_port > 0 {
        let bound = match u16::try_from(cfg.server_stats_port) {
            Ok(port) => runtime
                .block_on(TcpListener::bind((cfg.server_bind.as_str(), port)))
                .map_err(|e| e.to_string()),
            Err(_) => Err(format!("port {} is out of range", cfg.server_stats_port)),
        };
        match bound {
            Ok(listener) => {
                stats_port = listener.local_addr().ok().map(|a| a.port());
                logger::info(&format!("Started stats server on port {}", cfg.server_stats_port));
                runtime.spawn(stats_loop(listener, stats.clone()));
            }
            Err(e) => logger::error(&format!("Failed to start stats server: {e}")),
        }
    }

    let client_stat = Arc::new(Mutex::new(Bytes::from(server_stat(&world.client_stats_json()))));
    runtime.spawn(accept_loop(listener, acceptor, event_tx, client_stat.clone()));

    let game = GameLoop {
        world,
        events: event_rx,
        console: console_rx,
        conns: HashMap::new(),
        stats,
        client_stat,
        stats_interval: Duration::from_secs(cfg.server_stats_update),
        tracker: runtime.handle().clone(),
    };
    let game = std::thread::Builder::new()
        .name("game".into())
        .spawn(move || game.run())
        .map_err(|e| e.to_string())?;

    Ok(Running { port, stats_port, game, console: console_tx, runtime })
}

pub fn run(config_path: &Path) -> Result<i32, String> {
    let config = if config_path.exists() {
        Config::load(config_path)?
    } else {
        logger::warn(&format!("{} not found, using defaults", config_path.display()));
        Config::default()
    };
    logger::set_verbosity(config.log_verbosity);
    let data = Data::load(Path::new("data"));
    let running = start(config, data)?;

    let console = running.console.clone();
    std::thread::Builder::new()
        .name("console".into())
        .spawn(move || {
            for line in std::io::stdin().lock().split(b'\n').map_while(Result::ok) {
                let line = String::from_utf8_lossy(&line).trim_end_matches('\r').to_owned();
                if console.send(line).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;

    running.game.join().map_err(|_| "game thread panicked".to_owned())
}
