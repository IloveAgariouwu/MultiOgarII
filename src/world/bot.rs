use super::cell::{Cell, Kind, EAT_RATIO};
use super::player::Player;
use super::view::{Shared, ViewState};
use crate::core::math::Vec2;

const MAX_CELLS_FOR_SPLIT: usize = 8;
const SPLIT_COOLDOWN_TICKS: u32 = 15;
const SPLIT_REACH: f64 = 400.0;
const MOUSE_DISTANCE: f64 = 900.0;

pub(super) fn update_bot(s: &Shared, player: &Player, view: &mut ViewState) {
    view.split_cooldown = view.split_cooldown.saturating_sub(1);
    let mut largest: Option<&Cell> = None;
    for &slot in &player.units[0].cells {
        let current = &s.cells[slot];
        if largest.is_none_or(|cell| current.radius > cell.radius) {
            largest = Some(current);
        }
    }
    let Some(cell) = largest else { return };

    let mut result = Vec2::ZERO;
    for &(_, slot) in view.view_nodes.iter() {
        let node = &s.cells[slot];
        if node.owner == player.slot {
            continue;
        }
        let mut influence = bot_influence(s, player, node, cell);
        if influence == 0.0 {
            continue;
        }
        let displacement = node.pos - cell.pos;
        let mut distance = displacement.dist();
        if influence < 0.0 {
            distance -= cell.radius + node.radius;
        }
        distance = distance.max(1.0);
        influence /= distance;

        if influence > 0.0
            && node.kind != Kind::Food
            && cell.radius > node.radius * EAT_RATIO
            && view.split_cooldown == 0
            && player.units[0].cells.len() < MAX_CELLS_FOR_SPLIT
            && SPLIT_REACH - cell.radius / 2.0 - node.radius >= distance
        {
            view.split_cooldown = SPLIT_COOLDOWN_TICKS;
            view.bot_mouse = Some(node.pos);
            view.bot_split = true;
            return;
        }
        result += displacement.normalized() * influence;
    }
    view.bot_mouse = Some(cell.pos + result * MOUSE_DISTANCE);
}

fn bot_influence(s: &Shared, player: &Player, node: &Cell, cell: &Cell) -> f64 {
    match node.kind {
        Kind::Player => {
            let same_team = s.have_teams
                && s.players[node.owner as usize].as_ref().is_some_and(|o| o.team == player.team);
            if same_team {
                0.0
            } else if cell.radius > node.radius * EAT_RATIO {
                node.radius * 2.5
            } else if node.radius > cell.radius * EAT_RATIO {
                -node.radius
            } else {
                -(node.radius / cell.radius) / 3.0
            }
        }
        Kind::Food => 1.0,
        Kind::Ejected => {
            if cell.radius > node.radius * EAT_RATIO {
                node.radius
            } else {
                0.0
            }
        }
        Kind::Virus => {
            if cell.radius > node.radius * EAT_RATIO {
                if player.units[0].cells.len() == s.cfg.player_max_cells {
                    node.radius * 2.5
                } else {
                    -1.0
                }
            } else if node.is_mother && node.radius > cell.radius * EAT_RATIO {
                -1.0
            } else {
                0.0
            }
        }
    }
}
