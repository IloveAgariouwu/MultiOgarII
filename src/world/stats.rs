use super::player::PlayerKind;
use super::{World, TICKS_PER_SECOND, VERSION};

#[derive(Default)]
struct Population {
    alive: usize,
    spectators: usize,
    bots: usize,
    minions: usize,
}

impl World {
    fn population(&self) -> Population {
        let mut population = Population::default();
        for &slot in &self.order {
            let p = self.player(slot);
            if !p.connected {
                continue;
            }
            match p.kind {
                PlayerKind::Bot => population.bots += 1,
                PlayerKind::Minion { .. } => population.minions += 1,
                PlayerKind::Human if p.is_alive() => population.alive += 1,
                PlayerKind::Human => population.spectators += 1,
            }
        }
        population
    }

    pub fn stats_json(&self) -> String {
        let Population { alive, spectators, bots, minions } = self.population();
        serde_json::json!({
            "server_name": self.cfg.server_name,
            "server_chat": if self.cfg.server_chat != 0 { "true" } else { "false" },
            "border_width": self.border.width(),
            "border_height": self.border.height(),
            "gamemode": self.mode.name(),
            "max_players": self.cfg.server_max_connections,
            "current_players": alive + spectators,
            "alive": alive,
            "spectators": spectators,
            "bots": bots,
            "minions": minions,
            "update_time": format!("{:.3}", self.update_time_avg),
            "uptime": ((self.now - self.start_time) / 60000.0).round() as u64,
            "start_time": self.start_time as u64,
            "stats_time": self.now as u64,
        })
        .to_string()
    }

    pub fn client_stats_json(&self) -> String {
        let Population { alive, spectators, bots, .. } = self.population();
        serde_json::json!({
            "name": self.cfg.server_name,
            "mode": self.mode.name(),
            "uptime": ((self.now - self.start_time) / 1000.0).round() as u64,
            "update": format!("{:.3}", self.update_time_avg),
            "playersTotal": alive + spectators + bots,
            "playersAlive": alive + bots,
            "playersSpect": spectators,
            "playersLimit": self.cfg.server_max_connections,
        })
        .to_string()
    }

    pub fn tracker_ping(&self) -> Option<String> {
        if self.cfg.server_tracker == 0 || !self.ticks.is_multiple_of(TICKS_PER_SECOND * 30) {
            return None;
        }
        let Population { alive, spectators, .. } = self.population();
        let uptime = (self.now - self.boot_time) / 1000.0;
        Some(format!(
            "current_players={}&alive={alive}&spectators={spectators}&max_players={}&sport={}\
             &gamemode=[**] {}&agario=true&name=Unnamed Server&opp={} {}&uptime={uptime}\
             &version=MultiOgarII {VERSION}&start_time={}",
            alive + spectators,
            self.cfg.server_max_connections,
            self.cfg.server_port,
            self.mode.name(),
            match std::env::consts::OS {
                "windows" => "win32",
                "macos" => "darwin",
                other => other,
            },
            match std::env::consts::ARCH {
                "x86_64" => "x64",
                "aarch64" => "arm64",
                other => other,
            },
            self.start_time as u64,
        ))
    }
}
