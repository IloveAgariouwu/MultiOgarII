use super::cell::{Kind, Rgb};
use super::{Leaderboard, World};
use rand::Rng;

const TEAM_COLORS: [Rgb; 3] = [Rgb::new(223, 0, 0), Rgb::new(0, 223, 0), Rgb::new(0, 0, 223)];
const TEAM_COLOR_FUZZ: f64 = 32.0;
const MOTHER_CELLS: usize = 10;
pub const MOTHER_SIZE: f64 = 149.0;
const MOTHER_COLOR: Rgb = Rgb::new(0xCE, 0x63, 0x63);

const RAINBOW: [Rgb; 28] = [
    Rgb::new(255, 0, 0),
    Rgb::new(255, 32, 0),
    Rgb::new(255, 64, 0),
    Rgb::new(255, 96, 0),
    Rgb::new(255, 128, 0),
    Rgb::new(255, 160, 0),
    Rgb::new(255, 192, 0),
    Rgb::new(255, 224, 0),
    Rgb::new(255, 255, 0),
    Rgb::new(192, 255, 0),
    Rgb::new(128, 255, 0),
    Rgb::new(64, 255, 0),
    Rgb::new(0, 255, 0),
    Rgb::new(0, 192, 64),
    Rgb::new(0, 128, 128),
    Rgb::new(0, 64, 192),
    Rgb::new(0, 0, 255),
    Rgb::new(18, 0, 192),
    Rgb::new(37, 0, 128),
    Rgb::new(56, 0, 64),
    Rgb::new(75, 0, 130),
    Rgb::new(92, 0, 161),
    Rgb::new(109, 0, 192),
    Rgb::new(126, 0, 223),
    Rgb::new(143, 0, 255),
    Rgb::new(171, 0, 192),
    Rgb::new(199, 0, 128),
    Rgb::new(227, 0, 64),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeKind {
    Ffa,
    Teams,
    Experimental,
    Rainbow,
}

pub struct Mode {
    pub kind: ModeKind,
    pub mothers: Vec<u32>,
}

impl Mode {
    pub fn new(id: i32) -> Self {
        let kind = match id {
            1 => ModeKind::Teams,
            2 => ModeKind::Experimental,
            3 => ModeKind::Rainbow,
            _ => ModeKind::Ffa,
        };
        Self { kind, mothers: Vec::new() }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            ModeKind::Ffa => "Free For All",
            ModeKind::Teams => "Teams",
            ModeKind::Experimental => "Experimental",
            ModeKind::Rainbow => "Rainbow FFA",
        }
    }

    pub fn have_teams(&self) -> bool {
        self.kind == ModeKind::Teams
    }

    pub fn decay_mod(&self) -> f64 {
        if self.have_teams() {
            1.5
        } else {
            1.0
        }
    }
}

impl World {
    pub(super) fn mode_init(&mut self) {
        if self.mode.kind == ModeKind::Experimental {
            self.spawn_mother_cells();
        }
    }

    pub(super) fn mode_on_player_init(&mut self, slot: u32) {
        if self.mode.have_teams() {
            let team = self.rng.gen_range(0..TEAM_COLORS.len()) as u8;
            self.player_mut(slot).team = team;
        }
    }

    fn spawn_mother_cells(&mut self) {
        while self.mode.mothers.len() < MOTHER_CELLS {
            let wanted = self.random_pos();
            let pos = self.safe_position(wanted, MOTHER_SIZE);
            let slot = self.new_cell(Kind::Virus, pos, MOTHER_SIZE);
            self.unlist_virus(slot);
            self.cells[slot].is_mother = true;
            self.cells[slot].color = MOTHER_COLOR;
            self.mode.mothers.push(slot);
        }
    }

    pub fn on_player_spawn(&mut self, slot: u32, unit: u8) {
        let color = if self.mode.have_teams() {
            let base = TEAM_COLORS[self.player(slot).team as usize];
            Rgb::new(
                base.r + (self.rng.gen::<f64>() * TEAM_COLOR_FUZZ) as u8,
                base.g + (self.rng.gen::<f64>() * TEAM_COLOR_FUZZ) as u8,
                base.b + (self.rng.gen::<f64>() * TEAM_COLOR_FUZZ) as u8,
            )
        } else {
            self.random_color()
        };
        self.player_mut(slot).units[unit as usize].color = color;
        let pos = self.random_pos();
        self.spawn_player(slot, unit, pos);
    }

    pub(super) fn mode_on_tick(&mut self) {
        match self.mode.kind {
            ModeKind::Experimental => {
                for i in 0..self.mode.mothers.len() {
                    let slot = self.mode.mothers[i];
                    let interval = if self.cells[slot].radius <= MOTHER_SIZE {
                        self.rng.gen_range(25.0..50.0) as u64
                    } else {
                        2
                    };
                    if self.ticks.is_multiple_of(interval) {
                        self.update_mother_cell(slot);
                    }
                }
            }
            ModeKind::Rainbow => {
                for cell in self.cells.slots.iter_mut().filter(|c| c.alive) {
                    cell.color = RAINBOW[self.rng.gen_range(0..RAINBOW.len())];
                }
            }
            _ => {}
        }
    }

    pub(super) fn mode_on_mother_removed(&mut self, slot: u32) {
        self.mode.mothers.retain(|&m| m != slot);
        self.spawn_mother_cells();
    }

    pub(super) fn update_leaderboard(&mut self) {
        if self.mode.have_teams() {
            let mut mass = [0.0f64; TEAM_COLORS.len()];
            for &slot in &self.player_cells {
                let cell = &self.cells[slot];
                mass[self.player(cell.owner).team as usize] += cell.mass;
            }
            let total: f64 = mass.iter().sum();
            let shares = mass.iter().map(|m| if total > 0.0 { (m / total) as f32 } else { 0.0 });
            self.leaderboard = Leaderboard::Teams(shares.collect());
            self.largest = self
                .order
                .iter()
                .rev()
                .copied()
                .max_by(|&a, &b| self.player(a).score.total_cmp(&self.player(b).score));
            return;
        }

        let minions_listed = self.cfg.minions_on_leaderboard != 0;
        let mut rows: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|&slot| {
                let p = self.player(slot);
                p.is_alive() && p.connected && (minions_listed || !p.is_minion_kind())
            })
            .collect();
        rows.sort_by(|&a, &b| self.player(b).score.total_cmp(&self.player(a).score));
        self.largest = rows.first().copied();
        self.leaderboard = Leaderboard::Players(rows);
    }
}
