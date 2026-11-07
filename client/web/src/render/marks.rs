use std::collections::HashMap;

use ogar_core::world::{Cell, Kind, Tab, World};

const MAX_CELLS: usize = 16;
const HALF_SPLIT_MASS: f32 = 88.0;
pub(crate) const EAT_RATIO: f32 = 1.3;
const INDICATOR_REACH: f32 = 1200.0;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Mark {
    pub split_weight: f32,
    pub size_level: Option<u8>,
    pub show_mark: bool,
    pub can_eat: bool,
}

#[derive(Default)]
pub(crate) struct Marks {
    marks: HashMap<u32, Mark>,
    active: Vec<(u32, usize)>,
    ranks: Vec<usize>,
    half_biggest: Option<u32>,
}

fn heavier_cell<'a>(b: &'a Cell, c: &'a Cell) -> &'a Cell {
    if c.to.mass() > b.to.mass() {
        c
    } else {
        b
    }
}

fn remap(v: f32, from0: f32, from1: f32, to0: f32, to1: f32) -> f32 {
    let t = if from1 == from0 {
        1.0
    } else {
        (v - from0) / (from1 - from0)
    };
    to0 + (to1 - to0) * t.clamp(0.0, 1.0)
}

fn segment_distance(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((ax + dx * t - px).powi(2) + (ay + dy * t - py).powi(2)).sqrt()
}

pub(crate) struct Wanted {
    pub half_split: bool,
    pub mass_marker: bool,
    pub marker_extend: bool,
    pub marker_light: bool,
    pub split_indicator: bool,
}

impl Marks {
    pub(crate) fn mark_of(&self, id: u32) -> Mark {
        self.marks.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn is_half_biggest(&self, id: u32) -> bool {
        self.half_biggest == Some(id)
    }

    pub(crate) fn update_marks(&mut self, world: &World, aim: Option<(f32, f32)>, want: &Wanted) {
        self.marks.clear();
        self.half_biggest = None;
        self.active.clear();
        self.ranks.clear();
        let Some(owner) = world.active_owner() else {
            return;
        };
        let all = world.cells();
        self.active.extend(
            all.iter()
                .enumerate()
                .filter(|(_, c)| c.owner == owner && c.kind == Kind::Player)
                .map(|(slot, c)| (c.id, slot)),
        );
        if self.active.is_empty() {
            return;
        }
        self.active.sort_unstable_by_key(|&(id, _)| id);
        let first = &all[self.active[0].1];
        let biggest = [Tab::Main, Tab::Second]
            .into_iter()
            .flat_map(|tab| world.own(tab))
            .filter_map(|&id| world.cell(id))
            .filter(|c| c.owner == owner && c.kind == Kind::Player)
            .reduce(heavier_cell)
            .unwrap_or_else(|| {
                self.active
                    .iter()
                    .map(|&(_, slot)| &all[slot])
                    .fold(first, heavier_cell)
            });
        let count = self.active.len();
        let free = MAX_CELLS.saturating_sub(count);
        let mut possible = 0;
        for (i, &(id, slot)) in self.active.iter().enumerate() {
            let (weight, rank) = if possible < free {
                if all[slot].to.mass() >= 44.0 {
                    possible += 1;
                    (0.0, possible)
                } else {
                    (1.0, 0)
                }
            } else {
                let far = remap(i as f32, free as f32, (count - 1) as f32, 0.4, 1.0);
                (far, 0)
            };
            self.ranks.push(rank);
            self.marks.insert(
                id,
                Mark {
                    split_weight: weight,
                    ..Mark::default()
                },
            );
        }
        let (active, ranks) = (&self.active, &self.ranks);
        if want.half_split {
            let slots = MAX_CELLS as i64 - count as i64 - possible as i64;
            let biggest_rank = ranks[active
                .binary_search_by_key(&biggest.id, |&(id, _)| id)
                .expect("the biggest cell is active")];
            if biggest_rank > 0 && slots > 0 && biggest.to.mass() >= HALF_SPLIT_MASS {
                let (mut d, mut before) = (0i64, 0i64);
                for (&(_, slot), &rank) in active.iter().zip(ranks) {
                    if rank > 0 && all[slot].to.mass() >= HALF_SPLIT_MASS {
                        d += 1;
                        if rank < biggest_rank {
                            before += 1;
                        }
                    }
                }
                if before < slots && d + before >= slots {
                    self.half_biggest = Some(biggest.id);
                }
            }
        }
        let big = biggest.to.mass();
        if want.mass_marker {
            for c in all.iter().filter(|c| c.kind == Kind::Player) {
                let m = c.to.mass();
                let level = if want.marker_extend {
                    [
                        4.0 * big * EAT_RATIO,
                        2.0 * big * EAT_RATIO,
                        big * EAT_RATIO,
                        big / EAT_RATIO,
                        big / 2.0 * EAT_RATIO,
                        big / 2.0 / EAT_RATIO,
                        big / 4.0 / EAT_RATIO,
                        big / 8.0 * EAT_RATIO,
                        big / 8.0 / EAT_RATIO,
                    ]
                    .iter()
                    .position(|&t| m > t)
                    .map(|p| p as u8)
                } else {
                    let floor = if want.marker_light {
                        big / 4.0 / EAT_RATIO
                    } else {
                        big / 8.0 / EAT_RATIO
                    };
                    [
                        2.0 * big * EAT_RATIO,
                        big * EAT_RATIO,
                        big / EAT_RATIO,
                        big / 2.0 / EAT_RATIO,
                        floor,
                    ]
                    .iter()
                    .position(|&t| m > t)
                    .map(|p| p as u8)
                    .or(if want.marker_light { None } else { Some(5) })
                };
                self.marks.entry(c.id).or_default().size_level = level;
            }
        }
        if let (true, Some((ax, ay))) = (want.split_indicator, aim) {
            for (&(_, slot), &rank) in active.iter().zip(ranks) {
                let a = &all[slot];
                let eat_limit = 0.5 * 0.77 * a.to.mass();
                for t in all
                    .iter()
                    .filter(|t| t.kind == Kind::Player && t.owner != owner)
                {
                    if (a.to.x - t.to.x).abs() > INDICATOR_REACH
                        || (a.to.y - t.to.y).abs() > INDICATOR_REACH
                    {
                        continue;
                    }
                    let (dx, dy) = (ax - a.to.x, ay - a.to.y);
                    let len = (dx * dx + dy * dy).sqrt();
                    let (ex, ey) = if len > 0.0 {
                        (
                            a.to.x + dx / len * INDICATOR_REACH,
                            a.to.y + dy / len * INDICATOR_REACH,
                        )
                    } else {
                        (a.to.x, a.to.y)
                    };
                    if segment_distance(a.to.x, a.to.y, ex, ey, t.to.x, t.to.y) > a.to.r + t.to.r {
                        continue;
                    }
                    let dist = ((a.to.x - t.to.x).powi(2) + (a.to.y - t.to.y).powi(2)).sqrt();
                    let entry = self.marks.entry(t.id).or_default();
                    entry.show_mark = true;
                    if t.to.mass() < eat_limit && rank > 0 && dist < 720.0 + a.to.r {
                        entry.can_eat = true;
                    }
                }
            }
        }
    }
}
