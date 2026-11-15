use super::bot::update_bot;
use super::cell::{Cell, CellStore, Kind};
use super::mode::ModeKind;
use super::player::{Player, PlayerKind};
use super::{Leaderboard, World};
use crate::core::config::Config;
use crate::core::grid::DynamicGrid;
use crate::core::math::{Rect, Vec2};
use crate::net::compact::{self, IdList, Shown};
use crate::net::outbox::Outbox;
use crate::net::packets::{self, LbRow, NodeLabel, NodeRecord};

const LEADERBOARD_INTERVAL: u32 = 25;
const BORDER_INTERVAL: u32 = 20;

/// A cell the client knows: its id, its slot in the cell store, and, for compact
/// clients, what it was last told of it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Known {
    pub id: u32,
    pub slot: u32,
    pub sent: Shown,
}

#[derive(Default)]
pub struct ViewState {
    /// The cells the client knows, sorted by id.
    pub client_nodes: Vec<Known>,
    /// Built by each diff and then swapped with `client_nodes`.
    next_nodes: Vec<Known>,
    /// The cells in view as (id, slot), sorted by id.
    pub(super) view_nodes: Vec<(u32, u32)>,
    pub(super) hits: Vec<u32>,
    /// Indices into `client_nodes` of the cells to add and to update.
    add: Vec<u32>,
    update: Vec<u32>,
    delete: Vec<u32>,
    eaten: Vec<(u32, u32)>,
    /// Compact clients: updates whose cell changed, with what to tell.
    changed: Vec<(u32, Shown)>,
    /// Compact clients: per player slot, the pid and info version last told.
    announced: Vec<u64>,
    /// Compact clients get the leaderboard only when it changed.
    last_leaderboard: Vec<u8>,
    last_packet_size: usize,
    pub border_counter: u32,
    ticks_since_leaderboard: u32,
    pub(super) split_cooldown: u32,
    pub(super) bot_mouse: Option<Vec2>,
    pub(super) bot_split: bool,
    /// `World::ticks` at the last update sent; cells that changed since then
    /// are resent.
    pub(super) sent_at: u64,
}

impl ViewState {
    /// Forgets what the client was told, as after a restart, when pids and cell
    /// ids start over.
    pub(super) fn forget_client(&mut self) {
        self.client_nodes.clear();
        self.sent_at = 0;
        self.announced.clear();
        self.last_leaderboard.clear();
    }

    pub fn reset(&mut self) {
        self.client_nodes.clear();
        self.view_nodes.clear();
        self.announced.clear();
        self.last_leaderboard.clear();
        self.border_counter = 0;
        self.ticks_since_leaderboard = 0;
        self.split_cooldown = 0;
        self.bot_mouse = None;
        self.bot_split = false;
        self.sent_at = 0;
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
    ticks: u64,
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
            ticks: self.ticks,
        };
        for (view, player) in self.views.iter_mut().zip(self.players.iter()) {
            if let Some(player) = player {
                update_client(&shared, player, view);
            }
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

/// Fills `hits` with the grid slots of the cells inside the player's view boxes,
/// in grid order. Bots only need this; humans also need the id-sorted list.
fn collect_hits(s: &Shared, player: &Player, view: &mut ViewState) {
    view.hits.clear();
    let boxes = &player.view_boxes[..player.view_box_count as usize];
    for area in boxes {
        s.grid.query_into(*area, &mut view.hits);
    }
    if boxes.len() > 1 {
        view.hits.sort_unstable();
        view.hits.dedup();
    }
}

fn update_client(s: &Shared, player: &Player, view: &mut ViewState) {
    match player.kind {
        PlayerKind::Minion { .. } => {}
        PlayerKind::Bot => {
            // Bots think once every `serverBotUpdateInterval` ticks, spread over
            // the interval by slot so they do not all think on the same tick.
            let interval = s.cfg.server_bot_update_interval.max(1);
            if (s.ticks + player.slot as u64).is_multiple_of(interval as u64) {
                collect_hits(s, player, view);
                update_bot(s, player, view, interval);
            }
        }
        PlayerKind::Human => {
            if !player.is_minion {
                collect_view(s, player, view);
            }
            update_human(s, player, view);
        }
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

    // A connection that has not taken the previous update gets none this tick.
    // `client_nodes` still holds what that update told the client, so the next
    // diff covers both ticks and the client ends up in the same state.
    if conn.update_pending() {
        bump_leaderboard(s, player, view, conn);
        return;
    }

    diff_nodes(s, player, view);
    if player.compact {
        send_compact(s, player, view, conn);
    } else {
        send_update_nodes(s, player, view, conn);
    }
    bump_leaderboard(s, player, view, conn);
}

fn send_update_nodes(s: &Shared, player: &Player, view: &mut ViewState, conn: &Outbox) {
    let (sc, protocol) = (player.scramble, player.protocol);
    let (add, update) = (&view.add, &view.update);
    let estimate = 16
        + view.eaten.len() * 12
        + view.delete.len() * 4
        + (add.len() + update.len()) * 20
        + add.len() * 24;
    let mut buf = Vec::with_capacity(estimate.max(view.last_packet_size));
    packets::begin_update_nodes(&mut buf, &sc, &view.eaten);
    for &i in update {
        let cell = &s.cells[view.client_nodes[i as usize].slot];
        packets::put_node(&mut buf, protocol, &sc, &record(cell), None);
    }
    for &i in add {
        let cell = &s.cells[view.client_nodes[i as usize].slot];
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
    conn.push_update(buf);
}

fn shown(cell: &Cell, sc: &packets::Scramble) -> Shown {
    Shown::of(cell.pos.x, cell.pos.y, cell.radius, cell.color, sc)
}

fn compact_kind(cell: &Cell, viewer: &Player) -> u8 {
    match cell.kind {
        Kind::Food => compact::KIND_FOOD,
        Kind::Ejected => compact::KIND_EJECTED,
        Kind::Virus if cell.is_mother => compact::KIND_MOTHER,
        Kind::Virus => compact::KIND_VIRUS,
        Kind::Player if cell.owner == viewer.slot => compact::KIND_OWN + cell.unit,
        Kind::Player => compact::KIND_PLAYER,
    }
}

fn owner_of<'a>(s: &'a Shared, cell: &Cell) -> &'a Player {
    s.players[cell.owner as usize].as_ref().expect("a cell's owner exists")
}

/// Sends the world frame of the compact protocol, preceded by the names and
/// skins of players whose cells it adds and the client has not been told of.
fn send_compact(s: &Shared, player: &Player, view: &mut ViewState, conn: &Outbox) {
    let sc = player.scramble;
    let ViewState { client_nodes, add, update, delete, eaten, changed, announced, .. } = view;

    for &i in add.iter() {
        let cell = &s.cells[client_nodes[i as usize].slot];
        if cell.kind != Kind::Player {
            continue;
        }
        let owner = owner_of(s, cell);
        let key = u64::from(owner.pid) << 32 | u64::from(owner.info_version);
        let slot = cell.owner as usize;
        if announced.len() <= slot {
            announced.resize(slot + 1, 0);
        }
        if announced[slot] != key {
            announced[slot] = key;
            conn.push(compact::player(owner.pid, &owner.name, &owner.skin));
        }
    }

    changed.clear();
    for &i in update.iter() {
        let known = &client_nodes[i as usize];
        let now = shown(&s.cells[known.slot], &sc);
        if now != known.sent {
            changed.push((i, now));
        }
    }

    let estimate = 16 + eaten.len() * 8 + changed.len() * 6 + add.len() * 12 + delete.len() * 2;
    let buf = Vec::with_capacity(estimate.max(view.last_packet_size));
    let mut w = compact::World::begin(buf, player.center.x, player.center.y, &sc);

    w.count(eaten.len());
    let mut ids = IdList::new();
    for &(hunter, prey) in eaten.iter() {
        w.eat(&mut ids, compact::scramble_id(prey, &sc), compact::scramble_id(hunter, &sc));
    }

    w.count(changed.len());
    let mut ids = IdList::new();
    for &(i, now) in changed.iter() {
        let known = &mut client_nodes[i as usize];
        w.update(&mut ids, compact::scramble_id(known.id, &sc), &known.sent, &now);
        known.sent = now;
    }

    w.count(add.len());
    let mut ids = IdList::new();
    for &i in add.iter() {
        let known = &mut client_nodes[i as usize];
        let cell = &s.cells[known.slot];
        let now = shown(cell, &sc);
        let owner = (cell.kind == Kind::Player).then(|| owner_of(s, cell).pid);
        let kind = compact_kind(cell, player);
        w.add(&mut ids, compact::scramble_id(known.id, &sc), kind, &now, owner);
        known.sent = now;
    }

    w.count(delete.len());
    let mut ids = IdList::new();
    for &id in delete.iter() {
        w.remove(&mut ids, compact::scramble_id(id, &sc));
    }

    view.last_packet_size = w.buf.len();
    conn.push_update(w.buf);
}

fn bump_leaderboard(s: &Shared, player: &Player, view: &mut ViewState, conn: &Outbox) {
    view.ticks_since_leaderboard += 1;
    if view.ticks_since_leaderboard > LEADERBOARD_INTERVAL {
        view.ticks_since_leaderboard = 0;
        if let Some(frame) = leaderboard_frame(s, player) {
            if player.compact {
                if frame == view.last_leaderboard {
                    return;
                }
                view.last_leaderboard.clone_from(&frame);
            }
            conn.push(frame);
        }
    }
}

/// Compares the cells in view with those the client knows, walking both lists
/// in id order, and builds the new known list in the same order: cells to add,
/// cells to update, and the ids of cells eaten or gone from view. Owned cells
/// out of view stay known and are updated.
fn diff_nodes(s: &Shared, player: &Player, view: &mut ViewState) {
    view.add.clear();
    view.update.clear();
    view.delete.clear();
    view.eaten.clear();

    let since = std::mem::replace(&mut view.sent_at, s.ticks);
    let ViewState { client_nodes, next_nodes, view_nodes, add, update, delete, eaten, .. } = view;
    next_nodes.clear();

    let mut gone = |known: Known, next: &mut Vec<Known>, update: &mut Vec<u32>| {
        let cell = &s.cells[known.slot];
        if cell.id != known.id {
            // The cell was removed and its slot reused while updates to this
            // client were held back, so all it can be told is that it is gone.
            delete.push(known.id);
        } else if !cell.alive {
            eaten.push((cell.killer, known.id));
        } else if cell.owner != player.slot {
            delete.push(known.id);
        } else {
            update.push(next.len() as u32);
            next.push(known);
        }
    };

    let (mut v, mut c) = (0, 0);
    while v < view_nodes.len() && c < client_nodes.len() {
        let ((id, slot), known) = (view_nodes[v], client_nodes[c]);
        if id < known.id {
            add.push(next_nodes.len() as u32);
            next_nodes.push(Known { id, slot, sent: Shown::default() });
            v += 1;
        } else if id > known.id {
            gone(known, next_nodes, update);
            c += 1;
        } else {
            let cell = &s.cells[slot];
            // Food, ejected cells and viruses are resent only when they moved or
            // resized since the last update; player cells always change.
            if cell.changed_at >= since
                || cell.kind == Kind::Player
                || (s.rainbow && cell.kind == Kind::Food)
            {
                update.push(next_nodes.len() as u32);
            }
            next_nodes.push(known);
            v += 1;
            c += 1;
        }
    }
    for &(id, slot) in &view_nodes[v..] {
        add.push(next_nodes.len() as u32);
        next_nodes.push(Known { id, slot, sent: Shown::default() });
    }
    for &known in &client_nodes[c..] {
        gone(known, next_nodes, update);
    }
    std::mem::swap(client_nodes, next_nodes);
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
        Leaderboard::Players { ranked: list, place } => {
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
            let place = place.get(viewer.slot as usize).map_or(0, |&p| p as usize);
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
