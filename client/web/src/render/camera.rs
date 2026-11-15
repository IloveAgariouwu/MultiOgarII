use ogar_core::world::{Control, Tab, World};

use super::options::{CameraMode, Interpolation, Options};

const INITIAL_SCALE: f32 = 0.5;
const SCALE_KEEP: f32 = 0.86;
const WHEEL_STEP: f32 = 0.13;
const SCALE_MIN: f32 = 0.005;
const SCALE_MAX: f32 = 1.5;
const JUMP_TICKS: f32 = 2000.0;
const REPLAY_KEEP: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub css_scale: f32,
    pub half_view: (f32, f32),
    pub aim: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy)]
struct Jump {
    ticks: f32,
    x: f32,
    y: f32,
}

pub(crate) struct Sight {
    x: f32,
    y: f32,
    scale: f32,
    target_scale: f32,
    jump: Option<Jump>,
    spawned: bool,
    placed: bool,
    fit: Option<f32>,
    replay: Option<ReplayCamera>,
    stage: Option<Stage>,
    stage_zoom: f32,
    stage_zoom_target: f32,
    exact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stage {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReplayCamera {
    pub x: f32,
    pub y: f32,
    pub scale: Option<f32>,
}

fn smooth_toward(current: f32, target: f32, keep: f32) -> f32 {
    keep * current + (1.0 - keep) * target
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn weighted_center(world: &World, tab: Tab) -> (f32, f32, f32) {
    world
        .own(tab)
        .iter()
        .filter_map(|&id| world.cell(id))
        .fold((0.0, 0.0, 0.0), |(x, y, m), c| {
            let mass = c.to.mass();
            (x + c.to.x * mass, y + c.to.y * mass, m + mass)
        })
}

fn followed_tabs(world: &World) -> &'static [Tab] {
    match world.control() {
        Control {
            tabs_as_units: true,
            ..
        } => &[Tab::Main, Tab::Second],
        Control {
            tab: Tab::Second, ..
        } => &[Tab::Second],
        Control { tab: Tab::Main, .. } if world.own(Tab::Main).is_empty() => &[Tab::Second],
        Control { tab: Tab::Main, .. } => &[Tab::Main],
    }
}

fn unit_center(world: &World, now: f64, lerp_ms: f64) -> Option<(f32, f32)> {
    let (mut sx, mut sy, mut units) = (0.0, 0.0, 0.0);
    for &tab in followed_tabs(world) {
        let mut bounds: [Option<(f32, f32, f32, f32)>; 2] = [None; 2];
        for c in world.own(tab).iter().filter_map(|&id| world.cell(id)) {
            let at = c.at(now, lerp_ms);
            let b = (at.x - at.r, at.y - at.r, at.x + at.r, at.y + at.r);
            let unit = &mut bounds[(c.owner & 1) as usize];
            *unit = Some(unit.map_or(b, |o| {
                (o.0.min(b.0), o.1.min(b.1), o.2.max(b.2), o.3.max(b.3))
            }));
        }
        for (x0, y0, x1, y1) in bounds.into_iter().flatten() {
            sx += (x0 + x1) / 2.0;
            sy += (y0 + y1) / 2.0;
            units += 1.0;
        }
    }
    (units > 0.0).then(|| (sx / units, sy / units))
}

fn cell_center(world: &World, now: f64, lerp_ms: f64) -> Option<(f32, f32)> {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for &tab in followed_tabs(world) {
        for c in world.own(tab).iter().filter_map(|&id| world.cell(id)) {
            let at = c.at(now, lerp_ms);
            sx += at.x;
            sy += at.y;
            n += 1.0;
        }
    }
    (n > 0.0).then(|| (sx / n, sy / n))
}

impl Sight {
    pub(crate) fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: INITIAL_SCALE,
            target_scale: INITIAL_SCALE,
            jump: None,
            spawned: true,
            placed: false,
            fit: None,
            replay: None,
            stage: None,
            stage_zoom: 1.0,
            stage_zoom_target: 1.0,
            exact: false,
        }
    }

    pub(crate) fn reset_for_new_world(&mut self) {
        *self = Self {
            scale: self.scale,
            target_scale: self.target_scale,
            fit: self.fit,
            replay: self.replay,
            stage: self.stage,
            stage_zoom: self.stage_zoom,
            stage_zoom_target: self.stage_zoom_target,
            exact: self.exact,
            ..Self::new()
        };
    }

    pub(crate) fn on_spawned(&mut self) {
        self.spawned = true;
    }

    pub(crate) fn set_replay(&mut self, camera: Option<ReplayCamera>) {
        self.replay = camera;
    }

    pub(crate) fn set_stage(&mut self, stage: Option<Stage>) {
        self.stage = stage;
    }

    pub(crate) fn set_exact(&mut self, exact: bool) {
        self.exact = exact;
    }

    pub(crate) fn position_and_scale(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.scale)
    }

    pub(crate) fn set_fit(&mut self, margin: Option<f32>) {
        self.fit = margin;
    }

    fn fit_to_own(&mut self, world: &World, margin: f32, (css_w, css_h): (f32, f32)) -> bool {
        let bounds = [Tab::Main, Tab::Second]
            .into_iter()
            .flat_map(|tab| world.own(tab))
            .filter_map(|&id| world.cell(id))
            .map(|c| {
                (
                    c.to.x - c.to.r,
                    c.to.y - c.to.r,
                    c.to.x + c.to.r,
                    c.to.y + c.to.r,
                )
            })
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
        let Some((x0, y0, x1, y1)) = bounds else {
            return false;
        };
        self.x = (x0 + x1) / 2.0;
        self.y = (y0 + y1) / 2.0;
        let scale = (css_w / (x1 - x0 + 2.0 * margin))
            .min(css_h / (y1 - y0 + 2.0 * margin))
            .clamp(SCALE_MIN, SCALE_MAX);
        self.scale = scale;
        self.target_scale = scale;
        self.placed = true;
        true
    }

    pub(crate) fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.target_scale = scale;
    }

    pub(crate) fn zoom_by_wheel(&mut self, notches: f32, speed: f32) {
        self.zoom_by(1.0 + WHEEL_STEP * notches * speed);
    }

    pub(crate) fn zoom_by(&mut self, step: f32) {
        if self.stage.is_some() {
            self.stage_zoom_target = (self.stage_zoom_target * step).clamp(0.5, 5.0);
        } else {
            self.target_scale = (self.target_scale * step).clamp(SCALE_MIN, SCALE_MAX);
        }
    }

    pub(crate) fn screen_to_world(
        &self,
        css_x: f32,
        css_y: f32,
        (css_w, css_h): (f32, f32),
    ) -> (f32, f32) {
        (
            (css_x - css_w / 2.0) / self.scale + self.x,
            (css_y - css_h / 2.0) / self.scale + self.y,
        )
    }

    fn own_mass_center(&self, world: &World) -> Option<(f32, f32)> {
        let main = weighted_center(world, Tab::Main);
        let second = weighted_center(world, Tab::Second);
        let (x, y, m) = match world.control() {
            Control {
                tabs_as_units: true,
                ..
            } => (main.0 + second.0, main.1 + second.1, main.2 + second.2),
            Control {
                tab: Tab::Second, ..
            } if second.2 > 0.0 => second,
            Control { tab: Tab::Main, .. } if main.2 == 0.0 => second,
            _ => main,
        };
        (m > 0.0).then(|| (x / m, y / m))
    }

    pub(crate) fn follow_frame(
        &mut self,
        world: &World,
        now: f64,
        o: &Options,
        pointer: Option<(f32, f32)>,
        view: (f32, f32),
    ) {
        if let Some(margin) = self.fit {
            if self.fit_to_own(world, margin, view) {
                return;
            }
        }
        if let Some(stage) = self.stage {
            self.stage_zoom = smooth_toward(self.stage_zoom, self.stage_zoom_target, SCALE_KEEP);
            self.x = stage.x;
            self.y = stage.y;
            self.scale = stage.scale * self.stage_zoom;
            self.target_scale = self.scale;
            self.placed = true;
            return;
        }
        if let Some(r) = self.replay {
            self.follow_replay(world, r);
            return;
        }
        let follow_keep = 0.985 - o.camera_movement_speed * 5e-4;
        let lerp_ms = match o.interpolation {
            Interpolation::Linear | Interpolation::Timed | Interpolation::Softened => {
                world.lerp_ms()
            }
            Interpolation::Fast | Interpolation::Preceding | Interpolation::Chase => 0.0,
        };
        let own = self.own_mass_center(world);
        let (target, keep) = match own {
            Some(center) => {
                let (point, keep) = match o.camera_mode {
                    CameraMode::Default => (center, follow_keep),
                    CameraMode::UnitCenter => unit_center(world, now, lerp_ms)
                        .map_or((center, follow_keep), |p| {
                            (p, 1.0 - 1.0 / (102.0 - o.unit_camera_speed))
                        }),
                    CameraMode::CellCenter => cell_center(world, now, lerp_ms)
                        .map_or((center, follow_keep), |p| {
                            (p, 1.0 - 1.0 / (31.0 - o.cell_camera_speed))
                        }),
                };
                if self.spawned {
                    self.spawned = false;
                    self.jump = Some(Jump {
                        ticks: JUMP_TICKS,
                        x: point.0,
                        y: point.1,
                    });
                }
                match self.jump.as_mut() {
                    Some(j) => {
                        j.ticks -= 17.0;
                        let g = (1.0 - j.ticks / JUMP_TICKS).clamp(0.0, 1.0);
                        let jumped = (lerp(j.x, point.0, g), lerp(j.y, point.1, g));
                        let keep = lerp(0.94, 0.986, g);
                        if j.ticks <= 0.0 {
                            self.jump = None;
                        }
                        (Some(jumped), keep)
                    }
                    None => (Some(point), keep),
                }
            }
            None => {
                let target = world.server_view().map(|v| (v.x, v.y)).or(pointer);
                (target, follow_keep)
            }
        };
        let b = world.border();
        if !self.placed {
            let anchor = match own {
                Some(_) => target,
                None => world.server_view().map(|v| (v.x, v.y)),
            };
            if let Some((x, y)) = anchor.or_else(|| b.is_valid().then(|| b.center())) {
                self.x = x;
                self.y = y;
                self.placed = true;
            }
        } else if let Some((tx, ty)) = target {
            self.x = smooth_toward(self.x, tx, keep);
            self.y = smooth_toward(self.y, ty, keep);
        }
        self.scale = smooth_toward(self.scale, self.target_scale, SCALE_KEEP);
        if b.is_valid() {
            self.x = self.x.clamp(b.min_x, b.max_x);
            self.y = self.y.clamp(b.min_y, b.max_y);
        }
    }

    fn follow_replay(&mut self, world: &World, r: ReplayCamera) {
        if let Some(s) = r.scale {
            self.target_scale = s;
        }
        if self.exact || !self.placed {
            self.x = r.x;
            self.y = r.y;
            self.placed = true;
        } else {
            self.x = smooth_toward(self.x, r.x, REPLAY_KEEP);
            self.y = smooth_toward(self.y, r.y, REPLAY_KEEP);
        }
        self.scale = if self.exact {
            self.target_scale
        } else {
            smooth_toward(self.scale, self.target_scale, REPLAY_KEEP)
        };
        let b = world.border();
        if b.is_valid() {
            self.x = self.x.clamp(b.min_x, b.max_x);
            self.y = self.y.clamp(b.min_y, b.max_y);
        }
    }

    pub(crate) fn camera_for_view(&self, css_ratio: f32, view: (f32, f32)) -> Camera {
        Camera {
            x: self.x,
            y: self.y,
            scale: self.scale * css_ratio,
            css_scale: self.scale,
            half_view: (view.0 / 2.0 / self.scale, view.1 / 2.0 / self.scale),
            aim: None,
        }
    }
}
