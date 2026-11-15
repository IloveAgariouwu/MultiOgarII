use std::collections::hash_map::Entry;
use std::collections::HashMap;

use ogar_core::world::{Cell, Circle, Kind, World};

use super::options::{Interpolation, Options};

const LERP_MS_SLOW: f64 = 179.0;
const LERP_MS_FAST: f64 = 49.0;
const FAST_MS_SLOW: f64 = 120.0;
const FAST_MS_FAST: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Segment { ms: f64 },
    Tracked(Tracking),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tracking {
    Fast { ms: f64 },
    Preceding { keep: f32 },
    Chase,
}

fn lerp_range(t: f64, from: f64, to: f64) -> f64 {
    t * (to - from) + from
}

impl Mode {
    fn read(o: &Options) -> Self {
        let speed = f64::from(o.interpolation_speed);
        match o.interpolation {
            Interpolation::Fast => Mode::Tracked(Tracking::Fast {
                ms: lerp_range(speed, FAST_MS_SLOW, FAST_MS_FAST),
            }),
            Interpolation::Linear => Mode::Segment {
                ms: lerp_range(speed, LERP_MS_SLOW, LERP_MS_FAST),
            },
            Interpolation::Timed => Mode::Segment {
                ms: f64::from(o.timed_animation),
            },
            Interpolation::Softened => Mode::Segment {
                ms: f64::from(o.soften_animation),
            },
            Interpolation::Preceding => Mode::Tracked(Tracking::Preceding {
                keep: lerp_range(speed, f64::from(0.9_f32), f64::from(0.5_f32)) as f32,
            }),
            Interpolation::Chase => Mode::Tracked(Tracking::Chase),
        }
    }
}

#[derive(Clone, Copy)]
struct Glide {
    synced: u64,
    from: Circle,
    since: f64,
}

#[derive(Clone, Copy)]
enum Track {
    Fast(Glide),
    Preceding,
    Chase,
}

struct Shown {
    at: Circle,
    frame: u64,
    track: Track,
}

pub(crate) struct Motion {
    mode: Mode,
    sample_at: f64,
    frame_ms: f64,
    frame: u64,
    world: Option<(u64, u64)>,
    sync: u64,
    synced_at: f64,
    shown: HashMap<u32, Shown>,
}

impl Motion {
    pub(crate) fn new() -> Self {
        Self {
            mode: Mode::Segment { ms: LERP_MS_SLOW },
            sample_at: 0.0,
            frame_ms: 0.0,
            frame: 0,
            world: None,
            sync: 0,
            synced_at: 0.0,
            shown: HashMap::new(),
        }
    }

    pub(crate) fn begin_frame(
        &mut self,
        o: &Options,
        now: f64,
        frame_ms: f64,
        work_ms: f64,
        world: &World,
    ) -> f64 {
        self.mode = Mode::read(o);
        if self.world != Some((world.id(), world.epoch())) {
            self.world = Some((world.id(), world.epoch()));
            self.shown.clear();
        }
        self.sync = world.revision();
        self.synced_at = world.updated_at();
        self.frame += 1;
        self.frame_ms = frame_ms;
        self.sample_at = match self.mode {
            Mode::Segment { .. } => now + work_ms * f64::from(o.rendering_time_offset),
            Mode::Tracked(_) => now,
        };
        match self.mode {
            Mode::Segment { ms } | Mode::Tracked(Tracking::Fast { ms }) => ms,
            Mode::Tracked(Tracking::Preceding { .. } | Tracking::Chase) => 0.0,
        }
    }

    pub(crate) fn sample_at(&self) -> f64 {
        self.sample_at
    }

    pub(crate) fn is_segmented(&self) -> bool {
        matches!(
            self.mode,
            Mode::Segment { .. } | Mode::Tracked(Tracking::Fast { .. })
        )
    }

    pub(crate) fn position_of(&mut self, cell: &Cell, lerp_ms: f64) -> Circle {
        match self.mode {
            Mode::Segment { .. } => cell.at(self.sample_at, lerp_ms),
            Mode::Tracked(Tracking::Fast { .. }) if cell.kind == Kind::Food => {
                cell.at(self.sample_at, lerp_ms)
            }
            Mode::Tracked(tracking) => self.track_cell(tracking, cell),
        }
    }

    fn track_cell(&mut self, tracking: Tracking, cell: &Cell) -> Circle {
        let target = cell.to;
        let frame = self.frame;
        let (sync, synced_at) = (self.sync, self.synced_at);
        let s = match self.shown.entry(cell.id) {
            Entry::Occupied(e) if e.get().frame == frame => return e.get().at,
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(Shown {
                at: target,
                frame,
                track: match tracking {
                    Tracking::Fast { .. } => Track::Fast(Glide {
                        synced: sync,
                        from: target,
                        since: synced_at,
                    }),
                    Tracking::Preceding { .. } => Track::Preceding,
                    Tracking::Chase => Track::Chase,
                },
            }),
        };
        s.frame = frame;
        match tracking {
            Tracking::Fast { ms } => {
                let glide = match s.track {
                    Track::Fast(glide) if glide.synced == sync => glide,
                    _ => Glide {
                        synced: sync,
                        from: s.at,
                        since: synced_at,
                    },
                };
                let t = ((self.sample_at - glide.since) / ms).clamp(0.0, 1.0) as f32;
                s.at = glide.from.lerp(target, t);
                s.track = Track::Fast(glide);
            }
            Tracking::Preceding { keep } => {
                s.at = Circle {
                    x: keep * s.at.x + (1.0 - keep) * target.x,
                    y: keep * s.at.y + (1.0 - keep) * target.y,
                    r: keep * s.at.r + (1.0 - keep) * target.r,
                };
                s.track = Track::Preceding;
            }
            Tracking::Chase => {
                let g = 1.0 - 0.28_f32.powf((self.frame_ms / (1000.0 / 60.0)) as f32);
                let step = |cur: f32, to: f32| {
                    if (to - cur).abs() < 0.05 {
                        to
                    } else {
                        cur + (to - cur) * g
                    }
                };
                s.at = Circle {
                    x: step(s.at.x, target.x),
                    y: step(s.at.y, target.y),
                    r: s.at.r + (target.r - s.at.r) * g,
                };
                s.track = Track::Chase;
            }
        }
        s.at
    }

    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        self.shown.retain(|_, s| s.frame == frame);
    }
}
