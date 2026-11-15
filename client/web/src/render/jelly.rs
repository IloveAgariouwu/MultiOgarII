use std::collections::HashMap;
use std::f64::consts::TAU;

use ogar_core::world::Border;

const STEP_MS: f64 = 1000.0 / 240.0;
const STEP_COST_KEEP: f64 = 0.9;
const POINTS_PER_UNIT: f64 = 0.2;
const GRID: f64 = 50.0;
const TOUCH: f64 = 5.0;
const MAX_SPEED: f64 = 10.0;
const PULL: f64 = 12.0;
const SMOOTH_CENTER: f64 = 8.0;
const PUSH_KERNEL: [u16; 5] = [1, 2, 3, 2, 1];
const PUSH_KERNEL_SUM: u16 = {
    let mut sum = 0;
    let mut i = 0;
    while i < PUSH_KERNEL.len() {
        sum += PUSH_KERNEL[i];
        i += 1;
    }
    sum
};
pub(crate) const CORE: f32 = 0.8;
pub(crate) const LOD_PX: f32 = 20.0;
pub(crate) const VIEW_MARGIN: f32 = 40.0;
pub(crate) const SAMPLES: usize = 128;
pub(crate) const ROW: usize = SAMPLES + 1;

fn smooth(prev: f64, center: f64, next: f64) -> f64 {
    (prev + next + SMOOTH_CENTER * center) / (SMOOTH_CENTER + 2.0)
}

#[derive(Debug, Clone, Copy, Default)]
struct Point {
    r: f64,
    v: f64,
    prev: f64,
    x: f64,
    y: f64,
}

#[derive(Debug, Default)]
struct Body {
    x: f64,
    y: f64,
    size: f64,
    node: u32,
    virus: bool,
    detailed: bool,
    circle: bool,
    points: Vec<Point>,
    max: f64,
}

pub(crate) struct Jelly {
    bodies: Vec<Body>,
    free: Vec<usize>,
    cells: HashMap<u32, usize>,
    list: Vec<usize>,
    last_now: Option<f64>,
    acc: f64,
    alpha: f64,
    step_cost: Option<f64>,
    density: f64,
    rng: u64,
    order: Vec<usize>,
    grid_start: Vec<usize>,
    grid_cursor: Vec<usize>,
    grid_points: Vec<(f64, f64, usize)>,
    grid_origin: (f64, f64),
    grid_size: (usize, usize),
    hits: Vec<u8>,
    noise: Vec<f64>,
    spline: Vec<f64>,
}

pub(crate) fn radius(s: f32, r: f32, e: f32) -> f32 {
    let cs = CORE * r;
    let cd = CORE * r.min(e);
    if s <= cs {
        s * cd / cs
    } else if s <= r {
        cd + (s - cs) * (e - cd) / (r - cs)
    } else {
        s - r + e
    }
}

impl Jelly {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            bodies: Vec::new(),
            free: Vec::new(),
            cells: HashMap::new(),
            list: Vec::new(),
            last_now: None,
            acc: 0.0,
            alpha: 1.0,
            step_cost: None,
            density: POINTS_PER_UNIT,
            rng: seed | 1,
            order: Vec::new(),
            grid_start: Vec::new(),
            grid_cursor: Vec::new(),
            grid_points: Vec::new(),
            grid_origin: (0.0, 0.0),
            grid_size: (0, 0),
            hits: Vec::new(),
            noise: Vec::new(),
            spline: Vec::new(),
        }
    }

    fn next_random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(crate) fn begin_frame(&mut self) {
        self.list.clear();
    }

    pub(crate) fn track_cell(
        &mut self,
        cell: u32,
        at: (f32, f32, f32),
        virus: bool,
        detailed: bool,
    ) {
        let id = match self.cells.get(&cell) {
            Some(&id) => id,
            None => {
                let id = self.free.pop().unwrap_or_else(|| {
                    self.bodies.push(Body::default());
                    self.bodies.len() - 1
                });
                self.bodies[id] = Body {
                    circle: true,
                    ..Body::default()
                };
                self.cells.insert(cell, id);
                id
            }
        };
        let b = &mut self.bodies[id];
        b.x = f64::from(at.0);
        b.y = f64::from(at.1);
        b.size = f64::from(at.2);
        b.node = cell;
        b.virus = virus;
        b.detailed = detailed;
        self.list.push(id);
    }

    pub(crate) fn retain_cells(&mut self, alive: impl Fn(u32) -> bool) {
        let free = &mut self.free;
        self.cells.retain(|&cell, &mut id| {
            let keep = alive(cell);
            if !keep {
                free.push(id);
            }
            keep
        });
    }

    fn rotation_of(&self, id: usize, now: f64) -> f64 {
        let b = &self.bodies[id];
        if b.virus {
            0.0
        } else {
            (f64::from(b.node) / 1000.0 + now / 10000.0) % TAU
        }
    }

    pub(crate) fn advance_physics(
        &mut self,
        now: f64,
        border: Border,
        zoom: f32,
        clock: impl Fn() -> f64,
    ) {
        self.density = POINTS_PER_UNIT.min(f64::from(zoom));
        self.acc = match self.last_now {
            Some(last) => (self.acc + now - last).min(16.0 * STEP_MS),
            None => STEP_MS,
        };
        self.last_now = Some(now);
        let mut steps = (self.acc / STEP_MS).floor() as u32;
        self.acc -= f64::from(steps) * STEP_MS;
        if steps > 1 && self.step_cost.is_some_and(|c| c > STEP_MS) {
            steps = 1;
            self.acc = 0.0;
        }
        self.alpha = self.acc / STEP_MS;
        for _ in 0..steps {
            let start = clock();
            self.step_physics(now, border);
            let cost = clock() - start;
            self.step_cost = Some(
                self.step_cost
                    .map_or(cost, |c| c * STEP_COST_KEEP + cost * (1.0 - STEP_COST_KEEP)),
            );
        }
    }

    fn resize(&mut self, id: usize) {
        let density = self.density;
        let first = if self.bodies[id].points.is_empty() {
            self.next_random() - 0.5
        } else {
            0.0
        };
        let b = &mut self.bodies[id];
        let floor = if b.virus { 30 } else { 10 };
        let want = floor.max((b.size.floor() * density).floor() as usize);
        b.points.truncate(want);
        let last = b.points.last().copied().unwrap_or(Point {
            v: first,
            x: b.x,
            y: b.y,
            ..Point::default()
        });
        b.points.resize(want, last);
        let circle = !b.detailed;
        if b.circle && !circle {
            let n = b.points.len();
            let d = TAU / n as f64;
            for (i, p) in b.points.iter_mut().enumerate() {
                *p = Point {
                    x: b.x + (i as f64 * d).cos() * b.size,
                    y: b.y + (i as f64 * d).sin() * b.size,
                    ..Point::default()
                };
            }
        }
        b.circle = circle;
    }

    fn build_grid(&mut self) {
        self.order.clear();
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        let mut total = 0;
        for &id in &self.list {
            let b = &self.bodies[id];
            if b.circle {
                continue;
            }
            for p in &b.points {
                x0 = x0.min(p.x);
                x1 = x1.max(p.x);
                y0 = y0.min(p.y);
                y1 = y1.max(p.y);
            }
            total += b.points.len();
            self.order.push(id);
        }
        if total == 0 {
            self.grid_size = (0, 0);
            return;
        }
        let w = ((x1 - x0) / GRID).floor() as usize + 1;
        let h = ((y1 - y0) / GRID).floor() as usize + 1;
        self.grid_origin = (x0, y0);
        self.grid_size = (w, h);
        let cell_of = |x: f64, y: f64| {
            ((x - x0) / GRID).floor() as usize + ((y - y0) / GRID).floor() as usize * w
        };
        self.grid_start.clear();
        self.grid_start.resize(w * h + 1, 0);
        for &id in &self.order {
            for p in &self.bodies[id].points {
                self.grid_start[cell_of(p.x, p.y) + 1] += 1;
            }
        }
        for c in 1..=w * h {
            self.grid_start[c] += self.grid_start[c - 1];
        }
        self.grid_points.clear();
        self.grid_points.resize(total, (0.0, 0.0, 0));
        self.grid_cursor.clear();
        self.grid_cursor.extend_from_slice(&self.grid_start);
        for &id in &self.order {
            for p in &self.bodies[id].points {
                let c = cell_of(p.x, p.y);
                self.grid_points[self.grid_cursor[c]] = (p.x, p.y, id);
                self.grid_cursor[c] += 1;
            }
        }
    }

    fn touches_other_body(&self, id: usize, px: f64, py: f64) -> bool {
        let (w, h) = self.grid_size;
        let (gx0, gy0) = self.grid_origin;
        let span = |v: f64, origin: f64, len: usize| {
            let lo = ((v - TOUCH - origin) / GRID).floor().max(0.0) as usize;
            let hi = (((v + TOUCH - origin) / GRID).floor() as isize).min(len as isize - 1);
            (lo, hi)
        };
        let (cx0, cx1) = span(px, gx0, w);
        let (cy0, cy1) = span(py, gy0, h);
        if cx1 < 0 || cy1 < 0 {
            return false;
        }
        for cy in cy0..=cy1 as usize {
            let from = self.grid_start[cx0 + cy * w];
            let to = self.grid_start[cx1 as usize + cy * w + 1];
            for &(x, y, owner) in &self.grid_points[from..to] {
                if owner != id && (x - px).powi(2) + (y - py).powi(2) < TOUCH * TOUCH {
                    return true;
                }
            }
        }
        false
    }

    fn step_physics(&mut self, now: f64, border: Border) {
        for k in 0..self.list.len() {
            let id = self.list[k];
            self.resize(id);
        }
        self.build_grid();
        for k in 0..self.list.len() {
            let id = self.list[k];
            if self.bodies[id].circle {
                continue;
            }
            let n = self.bodies[id].points.len();
            let mut noise = std::mem::take(&mut self.noise);
            noise.clear();
            noise.extend((0..n).map(|_| self.next_random() - 0.5));
            let b = &mut self.bodies[id];
            for p in &mut b.points {
                p.prev = p.r;
            }
            for (i, jitter) in noise.iter().enumerate() {
                let prev = b.points[(i + n - 1) % n].v;
                let next = b.points[(i + 1) % n].v;
                let nv = ((b.points[i].v + jitter) * 0.7).clamp(-MAX_SPEED, MAX_SPEED);
                b.points[i].v = smooth(prev, nv, next);
            }
            let mut hits = std::mem::take(&mut self.hits);
            hits.clear();
            hits.resize(n + 4, 0);
            let mut any = false;
            for i in 0..n {
                let p = self.bodies[id].points[i];
                let outside = p.x < f64::from(border.min_x)
                    || p.y < f64::from(border.min_y)
                    || p.x > f64::from(border.max_x)
                    || p.y > f64::from(border.max_y);
                let hit = outside || self.touches_other_body(id, p.x, p.y);
                hits[i + 2] = u8::from(hit);
                any |= hit;
            }
            hits[0] = hits[n];
            hits[1] = hits[n + 1];
            hits[n + 2] = hits[2];
            hits[n + 3] = hits[3];
            let rot = self.rotation_of(id, now);
            let b = &mut self.bodies[id];
            let d = TAU / n as f64;
            let (cd, sd) = (d.cos(), d.sin());
            let (mut c, mut s) = (rot.cos(), rot.sin());
            let mut m: f64 = 0.0;
            for i in 0..n {
                let prev = b.points[(i + n - 1) % n].r;
                let next = b.points[(i + 1) % n].r;
                let p = &mut b.points[i];
                if any {
                    let push = f64::from(
                        PUSH_KERNEL
                            .iter()
                            .zip(&hits[i..i + PUSH_KERNEL.len()])
                            .map(|(&w, &h)| w * u16::from(h))
                            .sum::<u16>(),
                    ) / f64::from(PUSH_KERNEL_SUM);
                    if push > 0.0 {
                        p.v = p.v.min(0.0) - push;
                    }
                }
                let mut dr = (p.r + p.v).max(-b.size);
                dr = PULL * dr / (PULL + 1.0);
                dr = smooth(prev, dr, next);
                p.r = dr;
                let r = b.size + dr;
                let rr = if b.virus && i % 2 == 0 { r + 5.0 } else { r };
                p.x = b.x + c * rr;
                p.y = b.y + s * rr;
                m = m.max(rr);
                let t = c * cd - s * sd;
                s = s * cd + c * sd;
                c = t;
            }
            b.max = m;
            self.hits = hits;
            self.noise = noise;
        }
    }

    pub(crate) fn outline_row(&mut self, cell: u32, now: f64, out: &mut [f32]) -> bool {
        let Some(&id) = self.cells.get(&cell) else {
            return false;
        };
        if self.bodies[id].circle {
            return false;
        }
        let rot = self.rotation_of(id, now);
        let alpha = self.alpha;
        let b = &self.bodies[id];
        let n = b.points.len();
        self.spline.clear();
        self.spline.extend(
            b.points
                .iter()
                .map(|p| 1.0 + (p.prev + (p.r - p.prev) * alpha) / b.size),
        );
        let q = &self.spline;
        let mut high: f32 = 1.0;
        for (j, slot) in out[..SAMPLES].iter_mut().enumerate() {
            let angle = (j as f64 / SAMPLES as f64 * TAU - rot).rem_euclid(TAU);
            let t = angle / TAU * n as f64;
            let k = (t.floor() as usize) % n;
            let f = t - t.floor();
            let a = q[(k + n - 1) % n];
            let bq = q[k];
            let c = q[(k + 1) % n];
            let d = q[(k + 2) % n];
            let v = bq
                + 0.5
                    * f
                    * (c - a
                        + f * (2.0 * a - 5.0 * bq + 4.0 * c - d + f * (3.0 * (bq - c) + d - a)));
            *slot = v as f32;
            high = high.max(*slot);
        }
        out[SAMPLES] = high;
        true
    }
}
