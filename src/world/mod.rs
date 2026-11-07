mod bot;
pub mod cell;
mod chat;
pub mod commands;
pub mod data;
pub mod mode;
pub mod physics;
pub mod player;
pub mod spawn;
pub mod stats;
pub mod view;

pub use player::ConnInfo;

use crate::core::config::Config;
use crate::core::grid::DynamicGrid;
use crate::core::logger;
use crate::core::math::Rect;
use crate::net::outbox::CLOSE_NO_STATUS;
use crate::net::packets;
use cell::CellStore;
use data::Data;
use mode::Mode;
use physics::Manifold;
use player::{MinionSuspect, Player};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::sync::Arc;
use view::ViewState;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const TICKS_PER_SECOND: u64 = 25;
const RESTART_NOTICE_TICKS: u32 = 76;
const DECAY_TICK_PHASE: u64 = 3;
const LEADERBOARD_TICK_PHASE: u64 = 7;

pub enum Leaderboard {
    None,
    Players(Vec<u32>),
    Teams(Vec<f32>),
}

pub struct World {
    pub cfg: Arc<Config>,
    pub data: Data,
    pub rng: SmallRng,
    pub border: Rect,
    pub grid: DynamicGrid,
    pub cells: CellStore,
    pub virus_list: Vec<u32>,
    pub ejected_list: Vec<u32>,
    pub player_cells: Vec<u32>,
    pub food_count: usize,
    pub moving: Vec<u32>,
    graveyard: Vec<u32>,

    pub players: Vec<Option<Player>>,
    pub views: Vec<ViewState>,
    pub order: Vec<u32>,
    free_players: Vec<u32>,
    last_pid: u32,
    bot_count: u64,
    minion_test: Vec<MinionSuspect>,

    pub mode: Mode,
    pub leaderboard: Leaderboard,
    pub largest: Option<u32>,

    pub ticks: u64,
    pub running: bool,
    pub exit_requested: Option<i32>,
    restart_countdown: Option<u32>,
    restart_after_ticks: u64,

    pub now: f64,
    pub start_time: f64,
    pub boot_time: f64,
    pub console_aliases: bool,
    pub update_time_avg: f64,

    scratch_hits: Vec<u32>,
    scratch_manifolds: Vec<Manifold>,
}

impl World {
    pub fn new(cfg: Arc<Config>, data: Data, seed: u64, now: f64) -> Self {
        let border = world_border(&cfg);
        let mut world = Self {
            grid: new_grid(&cfg, border),
            mode: Mode::new(cfg.server_gamemode),
            restart_after_ticks: cfg.server_restart * 60 * TICKS_PER_SECOND,
            cfg,
            data,
            rng: SmallRng::seed_from_u64(seed),
            border,
            cells: CellStore::default(),
            virus_list: Vec::new(),
            ejected_list: Vec::new(),
            player_cells: Vec::new(),
            food_count: 0,
            moving: Vec::new(),
            graveyard: Vec::new(),
            players: Vec::new(),
            views: Vec::new(),
            order: Vec::new(),
            free_players: Vec::new(),
            last_pid: 0,
            bot_count: 0,
            minion_test: Vec::new(),
            leaderboard: Leaderboard::None,
            largest: None,
            ticks: 0,
            running: true,
            exit_requested: None,
            restart_countdown: None,
            now,
            start_time: now,
            boot_time: now,
            console_aliases: false,
            update_time_avg: 0.0,
            scratch_hits: Vec::new(),
            scratch_manifolds: Vec::new(),
        };
        world.mode_init();
        world.spawn_cells();
        for _ in 0..world.cfg.server_bots {
            world.add_bot();
        }
        world
    }

    pub fn player(&self, slot: u32) -> &Player {
        self.players[slot as usize].as_ref().expect("player slot in use")
    }

    pub fn player_mut(&mut self, slot: u32) -> &mut Player {
        self.players[slot as usize].as_mut().expect("player slot in use")
    }

    pub fn player_by_pid(&self, pid: u32) -> Option<u32> {
        self.order.iter().copied().find(|&slot| self.player(slot).pid == pid)
    }

    fn update_clients(&mut self) {
        let mut i = 0;
        while i < self.order.len() {
            let slot = self.order[i];
            self.check_connection(slot);
            if self.player(slot).removed {
                self.order.swap_remove(i);
                self.free_player(slot);
            } else {
                i += 1;
            }
        }
        for i in 0..self.order.len() {
            let slot = self.order[i];
            if self.player(slot).is_minion {
                continue;
            }
            if self.player(slot).connected {
                self.process_input(slot);
            }
            if self.player(slot).is_minion_kind() {
                continue;
            }
            self.update_view(slot);
            let player = self.player(slot);
            let (center, scale) = (player.center, player.scale);
            player.send(packets::update_position(center.x, center.y, scale, &player.scramble));
        }
        self.update_client_views();

        let interval = self.cfg.server_minion_interval;
        let now = self.now;
        self.minion_test.retain(|suspect| now - suspect.connected_at <= interval);
    }

    fn reclaim_removed_cells(&mut self) {
        while let Some(slot) = self.graveyard.pop() {
            self.cells.reclaim(slot);
        }
    }

    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if self.restart_after_ticks != 0 && self.ticks > self.restart_after_ticks && self.running {
            self.restart();
        }
        if let Some(left) = self.restart_countdown {
            self.restart_countdown = left.checked_sub(1).filter(|&left| left > 0);
            if self.restart_countdown.is_none() {
                self.finish_restart();
            }
        }
        if self.running {
            self.move_boosted_nodes();
            if (self.ticks + DECAY_TICK_PHASE).is_multiple_of(TICKS_PER_SECOND) {
                self.decay_player_cells();
            }
            self.update_player_cells();
            self.mode_on_tick();
            self.ticks += 1;
        }
        self.update_clients();
        if (self.ticks + LEADERBOARD_TICK_PHASE).is_multiple_of(TICKS_PER_SECOND) {
            self.update_leaderboard();
        }
        self.reclaim_removed_cells();
    }

    pub fn record_update_time(&mut self, ms: f64) {
        self.update_time_avg += 0.5 * (ms - self.update_time_avg);
    }

    pub fn restart(&mut self) {
        if self.running {
            self.running = false;
            self.broadcast("Server restarting...");
            self.restart_countdown = Some(RESTART_NOTICE_TICKS);
        }
    }

    fn finish_restart(&mut self) {
        for i in 0..self.order.len() {
            let slot = self.order[i];
            let player = self.player_mut(slot);
            player.kick(CLOSE_NO_STATUS, "");
            player.units[0].cells.clear();
            player.units[1].cells.clear();
            if !player.is_human() {
                player.removed = true;
            }
        }
        self.reclaim_removed_cells();
        self.cells.clear();
        self.grid = new_grid(&self.cfg, self.border);
        self.virus_list.clear();
        self.ejected_list.clear();
        self.player_cells.clear();
        self.moving.clear();
        self.food_count = 0;
        self.mode.mothers.clear();
        self.leaderboard = Leaderboard::None;
        self.largest = None;
        self.minion_test.clear();
        for view in &mut self.views {
            view.client_nodes.clear();
        }
        self.ticks = 0;
        self.last_pid = 0;
        self.start_time = self.now;
        self.running = true;
        for _ in 0..self.cfg.server_bots {
            self.add_bot();
        }
        if self.cfg.server_bots > 0 {
            logger::info(&format!("Added {} player bots", self.cfg.server_bots));
        }
        self.mode_init();
        self.spawn_cells();
        logger::info("Restarted");
    }
}

fn world_border(cfg: &Config) -> Rect {
    let (hw, hh) = (cfg.border_width / 2.0, cfg.border_height / 2.0);
    Rect::new(-hw, -hh, hw, hh)
}

fn new_grid(cfg: &Config, border: Rect) -> DynamicGrid {
    DynamicGrid::new(border, cfg.food_min_size, cfg.player_max_size)
}
