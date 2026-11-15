use std::collections::{HashMap, VecDeque};

use crate::world::Tab;

const TRACKED_CELLS: usize = 16;
const KEEP_MS: f64 = 3000.0;
const PRUNE_EVERY_MS: f64 = 500.0;
const DELAY_SAMPLES: usize = 32;
const MIN_DELAY_SAMPLES: usize = 4;
const CADENCE_SAMPLES: usize = 16;
const MIN_CADENCE_SAMPLES: usize = 4;
const DEFAULT_INTERVAL_MS: f64 = 40.0;
const MIN_INTERVAL_MS: f64 = 10.0;

fn position_key(x: f32, y: f32) -> u64 {
    u64::from(x.to_bits()) << 32 | u64::from(y.to_bits())
}

fn median(values: impl Iterator<Item = f64>, buf: &mut Vec<f64>) -> f64 {
    buf.clear();
    buf.extend(values);
    buf.sort_by(f64::total_cmp);
    buf[buf.len() / 2]
}

#[derive(Debug, Default, Clone)]
pub struct Echo {
    last: [HashMap<u32, u64>; 2],
    heard: [HashMap<(u32, u64), f64>; 2],
    tracked: Vec<u32>,
    samples: VecDeque<f64>,
    pruned_at: f64,
    scratch: Vec<f64>,
}

impl Echo {
    pub fn report(&mut self, tab: Tab, id: u32, x: f32, y: f32, now: f64) {
        if !self.tracked.contains(&id) {
            if self.tracked.len() >= TRACKED_CELLS {
                return;
            }
            self.tracked.push(id);
        }
        let i = tab.index();
        let at = position_key(x, y);
        let moved = self.last[i]
            .insert(id, at)
            .is_some_and(|before| before != at);
        if moved && !self.heard[i].contains_key(&(id, at)) {
            self.heard[i].insert((id, at), now);
            if let Some(&other) = self.heard[1 - i].get(&(id, at)) {
                let second_behind = match tab {
                    Tab::Main => other - now,
                    Tab::Second => now - other,
                };
                if self.samples.len() == DELAY_SAMPLES {
                    self.samples.pop_front();
                }
                self.samples.push_back(second_behind);
            }
        }
        if now - self.pruned_at >= PRUNE_EVERY_MS {
            self.prune(now);
        }
    }

    fn prune(&mut self, now: f64) {
        self.pruned_at = now;
        for heard in &mut self.heard {
            heard.retain(|_, at| now - *at < KEEP_MS);
        }
        let heard = &self.heard;
        self.tracked
            .retain(|id| heard.iter().any(|h| h.keys().any(|(cell, _)| cell == id)));
        let tracked = &self.tracked;
        for last in &mut self.last {
            last.retain(|id, _| tracked.contains(id));
        }
    }

    pub fn forget(&mut self, tab: Tab) {
        let i = tab.index();
        self.last[i].clear();
        self.heard[i].clear();
        self.samples.clear();
    }

    pub fn behind(&mut self) -> [f64; 2] {
        if self.samples.len() < MIN_DELAY_SAMPLES {
            return [0.0; 2];
        }
        let second_behind = median(self.samples.iter().copied(), &mut self.scratch);
        [(-second_behind).max(0.0), second_behind.max(0.0)]
    }
}

#[derive(Debug, Default, Clone)]
pub struct Cadence {
    intervals: VecDeque<f64>,
    burst: usize,
    span: f64,
    scratch: Vec<f64>,
}

impl Cadence {
    pub fn record(&mut self, interval: f64) {
        if interval < MIN_INTERVAL_MS && !self.intervals.is_empty() {
            let spread = self.burst.min(self.intervals.len());
            self.intervals.truncate(self.intervals.len() - spread);
            self.burst += 1;
            self.span += interval;
        } else {
            self.burst = 1;
            self.span = interval;
        }
        let share = self.span / self.burst as f64;
        for _ in 0..self.burst.min(CADENCE_SAMPLES) {
            if self.intervals.len() == CADENCE_SAMPLES {
                self.intervals.pop_front();
            }
            self.intervals.push_back(share);
        }
    }

    pub fn interval(&mut self) -> f64 {
        if self.intervals.len() < MIN_CADENCE_SAMPLES {
            return DEFAULT_INTERVAL_MS;
        }
        median(self.intervals.iter().copied(), &mut self.scratch).max(MIN_INTERVAL_MS)
    }
}
