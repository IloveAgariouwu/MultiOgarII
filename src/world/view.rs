use super::bot::update_bot;
use super::cell::{Cell, CellStore, Kind};
use super::mode::ModeKind;
use super::player::{Player, PlayerKind};
use super::{Leaderboard, World};
use crate::core::config::Config;
use crate::core::grid::DynamicGrid;
use crate::core::math::{Rect, Vec2};
use crate::net::packets::{self, LbRow, NodeLabel, NodeRecord};
use rayon::prelude::*;

const PARALLEL_MIN_CLIENTS: usize = 32;
const LEADERBOARD_INTERVAL: u32 = 25;
const BORDER_INTERVAL: u32 = 20;

#[derive(Default)]
pub struct ViewState {
    pub client_nodes: Vec<(u32, u32)>,
    pub(super) view_nodes: Vec<(u32, u32)>,
    hits: Vec<u32>,
    add: Vec<u32>,
    update: Vec<u32>,
    delete: Vec<u32>,
    eaten: Vec<(u32, u32)>,
    last_packet_size: usize,
    pub border_counter: u32,
    ticks_since_leaderboard: u32,
    pub(super) split_cooldown: u32,
    pub(super) bot_mouse: Option<Vec2>,
    pub(super) bot_split: bool,
}

impl ViewState {
    pub fn reset(&mut self) {
        self.client_nodes.clear();
        self.view_nodes.clear();
        self.border_counter = 0;
        self.ticks_since_leaderboard = 0;
        self.split_cooldown = 0;
        self.bot_mouse = None;
        self.bot_split = false;
    }
}

pub(super) struct Shared<'a> {
    grid: &'a DynamicGrid,
    pub(super) cells: &'a CellStore,
    pub(super) players: &'a [Option<Player>],
    pub(super) cfg: &'a Config,
    leaderboard: &'a Leaderboard,
    border: Rect,
    pub(super) have_teams: bool,
    rainbow: bool,
}

impl World {
    pub(in crate::world) fn update_view(&mut self, slot: u32) {
        let base_x = self.cfg.server_view_base_x + 100.0;
        let base_y = self.cfg.server_view_base_y + 100.0;
        let min_scale = self.cfg.server_min_scale;
        let border = self.border;
        let spectator_scale = self.cfg.server_spectator_scale;
        let target = self.largest;

        let cells = &self.cells;
        let (players_before, rest) = self.players.split_at_mut(slot as usize);
        let (me, players_after) = rest.split_first_mut().expect("slot in range");
        let player = me.as_mut().expect("player slot in use");

        let box_at = |center: Vec2, scale: f64| {
            Rect::around(center, base_x / scale / 2.0, base_y / scale / 2.0)
        };

        if player.is_alive() {
            let mut score = 0.0;
            let mut count = 0u8;
            let active = player.active as usize;
            for u in 0..2 {
                let unit = &player.units[u];
                if unit.cells.is_empty() {
                    continue;
                }
                let (mut radius_sum, mut center) = (0.0, Vec2::ZERO);
                for &slot in &unit.cells {
                    let cell = &cells[slot];
                    radius_sum += cell.radius;
                    score += cell.mass;
                    center += cell.pos;
                }
                center = center * (1.0 / unit.cells.len() as f64);
                let scale = (64.0 / radius_sum).min(1.0).powf(0.4).max(min_scale);
                if u == active {
                    player.center = center;
                    player.scale = scale;
                    player.active_box = count;
                }
                player.view_boxes[count as usize] = box_at(center, scale);
                count += 1;
            }
            player.view_box_count = count;
            player.score = score;
            return;
        }

        player.score = 0.0;
        if player.spectate {
            if let Some(t) = target.filter(|_| !player.free_roam) {
                let other = if t < slot {
                    players_before[t as usize].as_ref()
                } else if t > slot {
                    players_after[(t - slot - 1) as usize].as_ref()
                } else {
                    None
                };
                if let Some(p) = other {
                    player.center = clamp_to(p.center, &border);
                    player.scale = if p.is_alive() { p.scale } else { min_scale };
                } else if t == slot {
                    player.center = clamp_to(player.center, &border);
                    player.scale = min_scale;
                }
            } else {
                let to_mouse = player.mouse() - player.center;
                let dist = to_mouse.dist();
                if dist != 0.0 {
                    let step = (32.0 / dist).min(1.0);
                    player.center = clamp_to(player.center + to_mouse * step, &border);
                }
                player.scale = spectator_scale;
            }
        }
        player.view_boxes[0] = box_at(player.center, player.scale);
        player.view_box_count = 1;
    }

    pub(super) fn update_client_views(&mut self) {
        let shared = Shared {
            grid: &self.grid,
            cells: &self.cells,
            players: &self.players,
            cfg: &self.cfg,
            leaderboard: &self.leaderboard,
            border: self.border,
            have_teams: self.mode.have_teams(),
            rainbow: self.mode.kind == ModeKind::Rainbow,
        };
        let job = |(view, player): (&mut ViewState, &Option<Player>)| {
            if let Some(player) = player {
                update_client(&shared, player, view);
            }
        };
        if self.order.len() >= PARALLEL_MIN_CLIENTS {
            self.views.par_iter_mut().zip(self.players.par_iter()).with_min_len(4).for_each(job);
        } else {
            self.views.iter_mut().zip(self.players.iter()).for_each(job);
        }

        for i in 0..self.order.len() {
            let slot = self.order[i];
            let view = &mut self.views[slot as usize];
            let (mouse, split) = (view.bot_mouse.take(), std::mem::take(&mut view.bot_split));
            if mouse.is_some() || split {
                let player = self.player_mut(slot);
                if let Some(mouse) = mouse {
                    player.units[0].mouse = mouse;
                }
                player.split_requested |= split;
            }
        }
    }
}

/// Fills `view_nodes` with the cells inside the player's view boxes, sorted by id.
/// It runs after every player's input of this tick, so all players see the same world.
fn collect_view(s: &Shared, player: &Player, view: &mut ViewState) {
    let ViewState { view_nodes, hits, .. } = view;
    hits.clear();
    let boxes = &player.view_boxes[..player.view_box_count as usize];
    for area in boxes {
        s.grid.query_into(*area, hits);
    }
    view_nodes.clear();
    view_nodes.extend(hits.iter().map(|&node| (s.cells[node].id, node)));
    view_nodes.sort_unstable();
    if boxes.len() > 1 {
        view_nodes.dedup();
    }
}

fn update_client(s: &Shared, player: &Player, view: &mut ViewState) {
    if !player.is_minion && !player.is_minion_kind() {
        collect_view(s, player, view);
    }
    match player.kind {
        PlayerKind::Minion { .. } => {}
        PlayerKind::Bot => update_bot(s, player, view),
        PlayerKind::Human => update_human(s, player, view),
    }
}

fn update_human(s: &Shared, player: &Player, view: &mut ViewState) {
    let Some(conn) = &player.conn else { return };
    if player.is_minion || !player.connected || player.protocol == 0 {
        return;
    }
    let sc = player.scramble;
    let protocol = player.protocol;

    if s.cfg.server_scramble_level == 2 {
        if view.border_counter == 0 {
            let (b, v) = (s.border, player.view_boxes[player.active_box as usize]);
            let (half_w, half_h) = (v.width() / 2.0, v.height() / 2.0);
            let bound = [
                b.min_x.max(v.min_x - half_w),
                b.min_y.max(v.min_y - half_h),
                b.max_x.min(v.max_x + half_w),
                b.max_y.min(v.max_y + half_h),
            ];
            conn.push(packets::set_border(bound, &sc, None, protocol));
        }
        view.border_counter += 1;
        if view.border_counter >= BORDER_INTERVAL {
            view.border_counter = 0;
        }
    }

    diff_nodes(s, player, view);

    let (add, update) = (&view.add, &view.update);
    let estimate = 16
        + view.eaten.len() * 12
        + view.delete.len() * 4
        + (add.len() + update.len()) * 20
        + add.len() * 24;
    let mut buf = Vec::with_capacity(estimate.max(view.last_packet_size));
    packets::begin_update_nodes(&mut buf, &sc, &view.eaten);
    for &slot in update {
        packets::put_node(&mut buf, protocol, &sc, &record(&s.cells[slot]), None);
    }
    for &slot in add {
        let cell = &s.cells[slot];
        let label = label_of(s, cell, protocol);
        packets::put_node(&mut buf, protocol, &sc, &record(cell), Some(label));
    }
    packets::end_nodes(&mut buf);
    packets::put_remove_count(&mut buf, protocol, view.eaten.len() + view.delete.len());
    for &(_, prey) in &view.eaten {
        packets::put_removed(&mut buf, &sc, prey);
    }
    for &id in &view.delete {
        packets::put_removed(&mut buf, &sc, id);
    }
    view.last_packet_size = buf.len();
    conn.push(buf);

    view.ticks_since_leaderboard += 1;
    if view.ticks_since_leaderboard > LEADERBOARD_INTERVAL {
        view.ticks_since_leaderboard = 0;
        if let Some(frame) = leaderboard_frame(s, player) {
            conn.push(frame);
        }
    }
}

fn diff_nodes(s: &Shared, player: &Player, view: &mut ViewState) {
    view.add.clear();
    view.update.clear();
    view.delete.clear();
    view.eaten.clear();

    let ViewState { client_nodes, view_nodes, add, update, delete, eaten, .. } = view;
    let view_len = view_nodes.len();
    let mut keep_appended = false;

    let mut gone = |known: (u32, u32), view_nodes: &mut Vec<(u32, u32)>, update: &mut Vec<u32>| {
        let cell = &s.cells[known.1];
        if !cell.alive {
            eaten.push((cell.killer, known.0));
        } else if cell.owner != player.slot {
            delete.push(known.0);
        } else {
            update.push(known.1);
            view_nodes.push(known);
            keep_appended = true;
        }
    };

    let (mut v, mut c) = (0, 0);
    while v < view_len && c < client_nodes.len() {
        let (seen, known) = (view_nodes[v], client_nodes[c]);
        if seen.0 < known.0 {
            add.push(seen.1);
            v += 1;
        } else if seen.0 > known.0 {
            gone(known, view_nodes, update);
            c += 1;
        } else {
            let cell = &s.cells[seen.1];
            if cell.moving
                || matches!(cell.kind, Kind::Player | Kind::Virus)
                || (s.rainbow && cell.kind == Kind::Food)
            {
                update.push(seen.1);
            }
            v += 1;
            c += 1;
        }
    }
    add.extend(view_nodes[v..view_len].iter().map(|node| node.1));
    while c < client_nodes.len() {
        gone(client_nodes[c], view_nodes, update);
        c += 1;
    }

    if keep_appended {
        view_nodes.sort_unstable();
    }
    std::mem::swap(client_nodes, view_nodes);
}

fn record(cell: &Cell) -> NodeRecord {
    NodeRecord {
        id: cell.id,
        x: cell.pos.x,
        y: cell.pos.y,
        radius: cell.radius,
        color: cell.color,
        kind: cell.kind,
    }
}

fn label_of<'a>(s: &'a Shared, cell: &Cell, protocol: u32) -> NodeLabel<'a> {
    if !cell.has_owner() {
        return NodeLabel::default();
    }
    let owner = s.players[cell.owner as usize].as_ref().expect("a cell's owner exists");
    let skin = if protocol >= packets::PROTOCOL_P11 { &owner.skin_p11 } else { &owner.skin_utf8 };
    let name = if packets::uses_ucs2(protocol) { &owner.name_ucs2 } else { &owner.name_utf8 };
    NodeLabel { skin: skin.as_deref(), name: Some(name) }
}

fn leaderboard_frame(s: &Shared, viewer: &Player) -> Option<Vec<u8>> {
    match s.leaderboard {
        Leaderboard::None => None,
        Leaderboard::Teams(shares) => Some(packets::leaderboard_teams(shares)),
        Leaderboard::Players(list) => {
            let first_cell =
                viewer.active_unit().cells.first().map_or(0, |&slot| s.cells[slot].id);
            let rows: Vec<LbRow> = list
                .iter()
                .take(s.cfg.server_max_lb)
                .filter_map(|&slot| s.players[slot as usize].as_ref())
                .map(|p| {
                    let is_me = p.slot == viewer.slot;
                    LbRow {
                        name: &p.name,
                        name_utf8: &p.name_utf8,
                        name_ucs2: &p.name_ucs2,
                        is_me,
                        my_cell: if is_me { first_cell } else { 0 },
                    }
                })
                .collect();
            let place = list.iter().position(|&slot| slot == viewer.slot).map_or(0, |i| i + 1);
            Some(packets::leaderboard_ffa(
                &rows,
                viewer.protocol,
                &viewer.scramble,
                place,
                viewer.is_alive(),
            ))
        }
    }
}

fn clamp_to(p: Vec2, border: &Rect) -> Vec2 {
    Vec2::new(p.x.clamp(border.min_x, border.max_x), p.y.clamp(border.min_y, border.max_y))
}
