use super::cell::{Cell, Kind, EAT_RATIO};
use super::player::Player;
use super::view::{Shared, ViewState};
use crate::core::math::Vec2;

const MAX_CELLS_FOR_SPLIT: usize = 8;
const SPLIT_COOLDOWN_TICKS: u32 = 15;
const SPLIT_REACH: f64 = 400.0;
const MOUSE_DISTANCE: f64 = 900.0;

/// Steers a bot by the cells in `view.hits`, which is filled from the grid in
/// no particular order. The pull towards every visible cell is summed, and if
/// some cell is worth a split the one with the lowest id is chosen, which is the
/// cell the bot would meet first when walking its view in id order.
///
/// `elapsed` is the number of ticks since the bot last thought.
pub(super) fn update_bot(s: &Shared, player: &Player, view: &mut ViewState, elapsed: u32) {
    view.split_cooldown = view.split_cooldown.saturating_sub(elapsed);
    let mut largest: Option<&Cell> = None;
    for &slot in &player.units[0].cells {
        let current = &s.cells[slot];
        if largest.is_none_or(|cell| current.radius > cell.radius) {
            largest = Some(current);
        }
    }
    let Some(cell) = largest else { return };

    let can_split =
        view.split_cooldown == 0 && player.units[0].cells.len() < MAX_CELLS_FOR_SPLIT;
    let mut result = Vec2::ZERO;
    let mut split_at: Option<&Cell> = None;
    for &slot in &view.hits {
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

        if can_split
            && influence > 0.0
            && node.kind != Kind::Food
            && cell.radius > node.radius * EAT_RATIO
            && SPLIT_REACH - cell.radius / 2.0 - node.radius >= distance
            && split_at.is_none_or(|best| node.id < best.id)
        {
            split_at = Some(node);
        }
        result += displacement.normalized() * influence;
    }
    if let Some(target) = split_at {
        view.split_cooldown = SPLIT_COOLDOWN_TICKS;
        view.bot_mouse = Some(target.pos);
        view.bot_split = true;
        return;
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
