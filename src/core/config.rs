use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub log_verbosity: i32,

    pub server_timeout: f64,
    pub server_max_connections: usize,
    pub server_port: u16,
    pub server_bind: String,
    pub server_tracker: i32,
    pub server_gamemode: i32,
    pub server_bots: usize,
    pub server_view_base_x: f64,
    pub server_view_base_y: f64,
    pub server_min_scale: f64,
    pub server_spectator_scale: f64,
    pub server_stats_port: i32,
    pub server_stats_update: u64,
    pub mobile_physics: i32,
    pub bad_word_filter: i32,
    pub server_restart: u64,
    pub server_multibox: i32,

    #[serde(rename = "serverMaxLB")]
    pub server_max_lb: usize,
    pub server_chat: i32,
    pub server_chat_ascii: i32,
    pub separate_chat_for_teams: i32,
    pub server_name: String,
    pub server_welcome1: String,
    pub server_welcome2: String,
    pub client_bind: String,

    pub server_ip_limit: usize,
    pub server_minion_ignore_time: f64,
    pub server_minion_threshold: usize,
    pub server_minion_interval: f64,
    pub server_scramble_level: i32,
    pub player_bot_grow: i32,

    pub border_width: f64,
    pub border_height: f64,

    pub food_min_size: f64,
    pub food_max_size: f64,
    pub food_amount: usize,
    pub food_mass_grow: i32,

    pub virus_min_size: f64,
    pub virus_max_size: f64,
    pub virus_max_popped_size: f64,
    pub virus_equal_pop_size: i32,
    pub virus_amount: usize,
    pub virus_max_amount: usize,
    pub mother_cell_max_mass: f64,
    pub virus_velocity: f64,
    pub virus_max_cells: i32,

    pub eject_size: f64,
    pub eject_size_loss: f64,
    pub eject_cooldown: u64,
    pub eject_spawn_percent: f64,
    pub eject_virus: i32,
    pub eject_velocity: f64,

    pub player_min_size: f64,
    pub player_max_size: f64,
    pub player_min_split_size: f64,
    pub player_min_eject_size: f64,
    pub player_start_size: f64,
    pub player_max_cells: usize,
    pub player_speed: f64,
    pub player_decay_rate: f64,
    pub player_decay_cap: f64,
    pub player_recombine_time: f64,
    pub player_max_nick_length: usize,
    pub player_disconnect_time: f64,
    pub split_velocity: f64,

    pub minion_start_size: f64,
    pub minion_max_start_size: f64,
    pub minion_collide_team: i32,
    pub server_minions: usize,
    pub default_name: String,
    pub minions_on_leaderboard: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_verbosity: 4,

            server_timeout: 300.0,
            server_max_connections: 500,
            server_port: 8080,
            server_bind: "0.0.0.0".into(),
            server_tracker: 0,
            server_gamemode: 0,
            server_bots: 0,
            server_view_base_x: 1920.0,
            server_view_base_y: 1080.0,
            server_min_scale: 0.15,
            server_spectator_scale: 0.4,
            server_stats_port: 88,
            server_stats_update: 60,
            mobile_physics: 0,
            bad_word_filter: 1,
            server_restart: 0,
            server_multibox: 1,

            server_max_lb: 10,
            server_chat: 1,
            server_chat_ascii: 1,
            separate_chat_for_teams: 0,
            server_name: "MultiOgarII #1".into(),
            server_welcome1: "Welcome to MultiOgarII!".into(),
            server_welcome2: String::new(),
            client_bind: String::new(),

            server_ip_limit: 4,
            server_minion_ignore_time: 30.0,
            server_minion_threshold: 10,
            server_minion_interval: 1000.0,
            server_scramble_level: 0,
            player_bot_grow: 0,

            border_width: 14142.135623730952,
            border_height: 14142.135623730952,

            food_min_size: 10.0,
            food_max_size: 20.0,
            food_amount: 700,
            food_mass_grow: 1,

            virus_min_size: 100.0,
            virus_max_size: 141.421356237,
            virus_max_popped_size: 60.0,
            virus_equal_pop_size: 0,
            virus_amount: 50,
            virus_max_amount: 100,
            mother_cell_max_mass: 0.0,
            virus_velocity: 780.0,
            virus_max_cells: 0,

            eject_size: 36.06,
            eject_size_loss: 42.43,
            eject_cooldown: 3,
            eject_spawn_percent: 0.5,
            eject_virus: 0,
            eject_velocity: 780.0,

            player_min_size: 31.6227766017,
            player_max_size: 1500.0,
            player_min_split_size: 59.16079783,
            player_min_eject_size: 59.16079783,
            player_start_size: 31.6227766017,
            player_max_cells: 16,
            player_speed: 1.0,
            player_decay_rate: 0.002,
            player_decay_cap: 0.0,
            player_recombine_time: 30.0,
            player_max_nick_length: 15,
            player_disconnect_time: -1.0,
            split_velocity: 780.0,

            minion_start_size: 31.6227766017,
            minion_max_start_size: 31.6227766017,
            minion_collide_team: 0,
            server_minions: 0,
            default_name: "minion".into(),
            minions_on_leaderboard: 0,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
    }
}
