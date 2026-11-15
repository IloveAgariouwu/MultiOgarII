use std::collections::HashMap;

use serde::Deserialize;

use super::sample::identities;
use super::{Border, CellUpdate, Circle, Kind, Player, Rgb, Sym, Tab, World, AUTO_SPLIT_ALERT_MASS};

const FOCUS_EASE_MS: f64 = 250.0;

const FRAME_JSON: &str = include_str!("../../../data/preview-frame.json");
const DRIFT: f32 = 25.0;
const AIM_REACH: f32 = 0.38;
const AIM_STEP: f32 = 25.0;
const AIM_CLEARANCE: f32 = 60.0;
const BORDER_SPAN: f32 = 6000.0;
const BORDER_INSET: f32 = 0.3;
const VIRUS_SPAN: f32 = 1200.0;
const EJECT_MASS: f32 = 13.0;
const EJECTS: [(f32, f32); 4] = [
    (11150.0, 3350.0),
    (11280.0, 3420.0),
    (11180.0, 3520.0),
    (11300.0, 3600.0),
];
const BLUE_VIRUS_MASS: f32 = 50.0;
const BLUE_VIRUS_OFFSET: (f32, f32) = (380.0, -220.0);
const JELLY_CELLS: [(f32, f32, f32); 3] = [
    (4880.0, 3000.0, 40.0),
    (4944.0, 3000.0, 34.0),
    (4918.0, 3055.0, 37.0),
];
const JELLY_SPAN: f32 = 300.0;
const MARKER_CELLS: [(f32, f32, f32); 8] = [
    (3000.0, 10500.0, 6.0),
    (8000.0, 10800.0, 3.6),
    (12100.0, 10300.0, 1.8),
    (2700.0, 4500.0, 1.0),
    (4900.0, 2000.0, 0.7),
    (12900.0, 6900.0, 0.5),
    (12900.0, 1400.0, 0.28),
    (5000.0, 6500.0, 0.175),
];
const FIELD_MARGIN: f32 = 1.04;
const NAME_COLOR: Rgb = Rgb(255, 255, 255);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    featured: [u32; 2],
    border: [f32; 4],
    aim: [f32; 2],
    active: u32,
    players: Vec<u32>,
    own: [Vec<u32>; 2],
    cells: Vec<FrameCell>,
}

fn kind_of_code(code: u8) -> Kind {
    match code {
        0 => Kind::Player,
        1 => Kind::Food,
        2 => Kind::Virus,
        3 => Kind::Eject,
        other => unreachable!("preview frame cell kind {other} is not a known kind"),
    }
}

fn unpack_rgb(packed: u32) -> Rgb {
    Rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(from = "(u8, u32, f32, f32, f32, u32, u32)")]
struct FrameCell {
    kind: u8,
    id: u32,
    x: f32,
    y: f32,
    r: f32,
    color: u32,
    owner: u32,
}

impl From<(u8, u32, f32, f32, f32, u32, u32)> for FrameCell {
    fn from((kind, id, x, y, r, color, owner): (u8, u32, f32, f32, f32, u32, u32)) -> Self {
        Self {
            kind,
            id,
            x,
            y,
            r,
            color,
            owner,
        }
    }
}

impl Frame {
    fn bystanders(&self, me: u32) -> impl Iterator<Item = u32> + '_ {
        self.players
            .iter()
            .copied()
            .filter(move |&o| !self.featured.contains(&o) && o != me)
    }
}

fn bounds_of<'a>(cells: impl Iterator<Item = &'a FrameCell>) -> (f32, f32, f32, f32) {
    cells.fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(x0, y0, x1, y1), c| {
            (
                x0.min(c.x - c.r),
                y0.min(c.y - c.r),
                x1.max(c.x + c.r),
                y1.max(c.y + c.r),
            )
        },
    )
}

fn mass_center<'a>(cells: impl Iterator<Item = &'a FrameCell>) -> (f32, f32) {
    let (sx, sy, sw) = cells.fold((0.0, 0.0, 0.0), |(sx, sy, sw), c| {
        let w = c.r * c.r;
        (sx + c.x * w, sy + c.y * w, sw + w)
    });
    (sx / sw, sy / sw)
}

fn scene_shot(f: &Frame) -> Shot {
    let own = |c: &&FrameCell| f.own.iter().any(|ids| ids.contains(&c.id));
    let (sx0, sy0, sx1, sy1) = bounds_of(f.cells.iter().filter(own));
    let (x, y) = ((sx0 + sx1) / 2.0, (sy0 + sy1) / 2.0);
    let others = f.featured.map(|player| {
        let (cx, cy) = mass_center(f.cells.iter().filter(|c| c.owner & !1 == player));
        (cx - x).abs().max((cy - y).abs())
    });
    let reach = [x - sx0, sx1 - x, y - sy0, sy1 - y]
        .into_iter()
        .chain(others)
        .fold(0.0, f32::max);
    Shot {
        x,
        y,
        span: 2.0 * reach * 1.05,
    }
}

fn aim_offset(f: &Frame, scene: Shot) -> (f32, f32) {
    let (dx, dy) = (f.aim[0] - scene.x, f.aim[1] - scene.y);
    let length = dx.hypot(dy);
    let (ux, uy) = (dx / length, dy / length);
    let reach = scene.span * AIM_REACH;
    let clear = |x: f32, y: f32| {
        f.cells
            .iter()
            .all(|c| (c.x - x).hypot(c.y - y) > c.r + AIM_CLEARANCE)
    };
    let own = |c: &&FrameCell| f.own.iter().any(|ids| ids.contains(&c.id));
    let start = f
        .cells
        .iter()
        .filter(own)
        .map(|c| ((c.x - scene.x) * ux + (c.y - scene.y) * uy) + c.r)
        .fold(0.0, f32::max);
    let mut d = start;
    while d < reach && !clear(scene.x + ux * d, scene.y + uy * d) {
        d += AIM_STEP;
    }
    let d = d.min(reach);
    (ux * d / scene.span, uy * d / scene.span)
}

fn biggest_own_cell(f: &Frame) -> u32 {
    f.cells
        .iter()
        .filter(|c| f.own[0].contains(&c.id))
        .max_by(|a, b| a.r.total_cmp(&b.r))
        .map(|c| c.id)
        .expect("bundled preview frame has own cells")
}

fn virus_shot(f: &Frame, around: Shot) -> Shot {
    let (x, y) = f
        .cells
        .iter()
        .filter(|c| kind_of_code(c.kind) == Kind::Virus)
        .map(|c| (c.x, c.y))
        .min_by(|a, b| {
            (a.0 - around.x)
                .hypot(a.1 - around.y)
                .total_cmp(&(b.0 - around.x).hypot(b.1 - around.y))
        })
        .expect("bundled preview frame has a virus");
    Shot {
        x,
        y,
        span: VIRUS_SPAN,
    }
}

fn extra_cells(f: &Frame, own_cell: u32, virus: Shot) -> Vec<FrameCell> {
    let (own_color, owner) = f
        .cells
        .iter()
        .find(|c| c.id == own_cell)
        .map(|c| (c.color, c.owner))
        .expect("the alert cell is in the frame");
    let virus_color = f
        .cells
        .iter()
        .find(|c| kind_of_code(c.kind) == Kind::Virus)
        .map(|c| c.color)
        .expect("bundled preview frame has a virus");
    let first_id = f
        .cells
        .iter()
        .map(|c| c.id)
        .max()
        .expect("bundled preview frame has cells")
        + 1;
    let eject_r = Circle::radius_of(EJECT_MASS);
    let ejects = EJECTS
        .iter()
        .map(|&(x, y)| (3, x, y, eject_r, own_color, owner));
    let blue = std::iter::once((
        2,
        virus.x + BLUE_VIRUS_OFFSET.0,
        virus.y + BLUE_VIRUS_OFFSET.1,
        Circle::radius_of(BLUE_VIRUS_MASS),
        virus_color,
        0,
    ));
    let mut bystanders = f.bystanders(owner & !1);
    let jelly = JELLY_CELLS.iter().map(|&(x, y, r)| {
        let bystander = bystanders
            .next()
            .expect("the frame has a bystander per jelly cell");
        let color = f
            .cells
            .iter()
            .find(|c| c.owner == bystander)
            .map(|c| c.color)
            .expect("every bystander has a cell");
        (0, x, y, r, color, bystander)
    });
    ejects
        .chain(blue)
        .chain(jelly)
        .zip(first_id..)
        .map(|((kind, x, y, r, color, owner), id)| FrameCell {
            kind,
            id,
            x,
            y,
            r,
            color,
            owner,
        })
        .collect()
}

fn jelly_shot() -> Shot {
    let n = JELLY_CELLS.len() as f32;
    let (x, y) = JELLY_CELLS
        .iter()
        .fold((0.0, 0.0), |(x, y), c| (x + c.0 / n, y + c.1 / n));
    Shot {
        x,
        y,
        span: JELLY_SPAN,
    }
}

fn marker_cells(f: &Frame, first_id: u32) -> Vec<FrameCell> {
    let biggest = f
        .cells
        .iter()
        .filter(|c| c.owner == f.active)
        .map(|c| c.r)
        .fold(0.0, f32::max);
    let big = biggest * biggest / 100.0;
    let mut bystanders = f.bystanders(f.active & !1).skip(JELLY_CELLS.len());
    MARKER_CELLS
        .iter()
        .zip(first_id..)
        .map(|(&(x, y, share), id)| {
            let owner = bystanders
                .next()
                .expect("the frame has a bystander per marker cell");
            let color = f
                .cells
                .iter()
                .find(|c| c.owner == owner)
                .map(|c| c.color)
                .expect("every bystander has a cell");
            FrameCell {
                kind: 0,
                id,
                x,
                y,
                r: Circle::radius_of(big * share),
                color,
                owner,
            }
        })
        .collect()
}

fn field_shot(f: &Frame) -> Shot {
    let [min_x, min_y, max_x, max_y] = f.border;
    Shot {
        x: (min_x + max_x) / 2.0,
        y: (min_y + max_y) / 2.0,
        span: (max_x - min_x).max(max_y - min_y) * FIELD_MARGIN,
    }
}

fn border_shot(f: &Frame, around: Shot) -> Shot {
    let [min_x, min_y, max_x, max_y] = f.border;
    let inward = BORDER_SPAN * (0.5 - BORDER_INSET);
    let toward = |near: f32, far: f32, at: f32| {
        if at - near < far - at {
            near + inward
        } else {
            far - inward
        }
    };
    Shot {
        x: toward(min_x, max_x, around.x),
        y: toward(min_y, max_y, around.y),
        span: BORDER_SPAN,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Scene,
    Virus,
    AutoSplit,
    Border,
    Jelly,
    Marker,
    Grid,
    Field,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub x: f32,
    pub y: f32,
    pub span: f32,
}

struct Transition {
    from: Shot,
    since: f64,
}

pub struct SelfIdentity<'a> {
    pub name: &'a str,
    pub skins: [&'a str; 2],
    pub external: [&'a str; 2],
}

pub struct PreviewScene {
    frame: Frame,
    colors: HashMap<u32, Rgb>,
    extras: Vec<FrameCell>,
    markers: Vec<FrameCell>,
    field_shot: Shot,
    last_tick: Option<f64>,
    scene_shot: Shot,
    aim_offset: (f32, f32),
    virus_shot: Shot,
    border_shot: Shot,
    alert_cell: u32,
    focus: Focus,
    transition: Option<Transition>,
}

impl PreviewScene {
    pub fn create(world: &mut World, seed: u64) -> Self {
        let frame: Frame = serde_json::from_str(FRAME_JSON).expect("bundled preview frame parses");
        world.clear();
        let [min_x, min_y, max_x, max_y] = frame.border;
        world.set_border(Border {
            min_x,
            min_y,
            max_x,
            max_y,
        });
        world.set_active_owner(Some(frame.active));
        let people = identities(seed, frame.players.len());
        let me = frame.active & !1;
        let mut colors = HashMap::new();
        for (&owner, person) in frame.players.iter().zip(&people) {
            let player = Player {
                name: world.intern(&person.name),
                skins: [Sym::NONE; 2],
                team_color: NAME_COLOR,
            };
            world.set_player(owner, player);
            if owner != me {
                colors.insert(owner, person.rgb);
            }
        }
        let scene_shot = scene_shot(&frame);
        let aim_offset = aim_offset(&frame, scene_shot);
        let virus_shot = virus_shot(&frame, scene_shot);
        let border_shot = border_shot(&frame, scene_shot);
        let alert_cell = biggest_own_cell(&frame);
        let extras = extra_cells(&frame, alert_cell, virus_shot);
        let field_shot = field_shot(&frame);
        let last_extra = extras.last().expect("the preview stages extra cells").id;
        let markers = marker_cells(&frame, last_extra + 1);
        let scene = Self {
            frame,
            colors,
            extras,
            markers,
            field_shot,
            last_tick: None,
            scene_shot,
            aim_offset,
            virus_shot,
            border_shot,
            alert_cell,
            focus: Focus::Scene,
            transition: None,
        };
        world.begin_update(0.0);
        scene.write_to_world(world, 0.0);
        scene
    }

    pub fn update_frame(&mut self, world: &mut World, now: f64) {
        if self.last_tick.is_some_and(|t| now - t < 40.0) {
            return;
        }
        self.last_tick = Some(now);
        world.begin_update(now);
        self.write_to_world(world, now as f32);
    }

    pub fn focus_camera(&mut self, focus: Focus, now: f64) -> Shot {
        if focus != self.focus {
            self.transition = Some(Transition {
                from: self.current_shot(now),
                since: now,
            });
            self.focus = focus;
        }
        self.current_shot(now)
    }

    fn current_shot(&self, now: f64) -> Shot {
        let to = match self.focus {
            Focus::Scene | Focus::AutoSplit | Focus::Second => self.scene_shot,
            Focus::Virus => self.virus_shot,
            Focus::Border => self.border_shot,
            Focus::Jelly => jelly_shot(),
            Focus::Marker | Focus::Grid | Focus::Field => self.field_shot,
        };
        let Some(t) = &self.transition else {
            return to;
        };
        let u = ((now - t.since) / FOCUS_EASE_MS).clamp(0.0, 1.0) as f32;
        let e = 1.0 - (1.0 - u).powi(3);
        Shot {
            x: t.from.x + (to.x - t.from.x) * e,
            y: t.from.y + (to.y - t.from.y) * e,
            span: t.from.span * (to.span / t.from.span).powf(e),
        }
    }

    pub fn set_self_identity(&self, world: &mut World, me: &SelfIdentity<'_>) {
        let owner = self.frame.active;
        let current = *world
            .player(owner)
            .expect("the preview registers the active player");
        let player = Player {
            name: world.intern(me.name),
            skins: me.skins.map(|s| world.intern(s)),
            ..current
        };
        world.set_player(owner, player);
        let external = me.external.map(|s| world.intern(s));
        world.set_external_skins(Tab::Main, external);
    }

    fn color(&self, kind: Kind, owner: u32, recorded: u32) -> Rgb {
        match self.colors.get(&(owner & !1)) {
            Some(&c) if kind == Kind::Player => c,
            _ => unpack_rgb(recorded),
        }
    }

    pub fn aim_point_for(&self, shot: Shot) -> (f32, f32) {
        (
            shot.x + self.aim_offset.0 * shot.span,
            shot.y + self.aim_offset.1 * shot.span,
        )
    }

    fn write_to_world(&self, world: &mut World, t: f32) {
        world.clear_own(Tab::Main);
        world.clear_own(Tab::Second);
        let marking = self.focus == Focus::Marker;
        let me = self.frame.active & !1;
        let recorded = self.frame.cells.iter().map(|c| (c, true));
        let staged = self.extras.iter().map(|c| (c, false));
        for (
            &FrameCell {
                kind: code,
                id,
                x,
                y,
                r,
                color,
                owner,
            },
            drifts,
        ) in recorded.chain(staged)
        {
            let hidden = match self.focus {
                Focus::Marker => !drifts || owner & !1 != me,
                Focus::Grid | Focus::Border | Focus::Field => true,
                _ => false,
            };
            if hidden {
                world.remove(id, None);
                continue;
            }
            let r = if self.focus == Focus::AutoSplit && id == self.alert_cell {
                Circle::radius_of(AUTO_SPLIT_ALERT_MASS * 1.05)
            } else {
                r
            };
            let kind = kind_of_code(code);
            let phase = (id % 997) as f32;
            let (x, y) = if drifts && kind == Kind::Player {
                (
                    x + (t / 2300.0 + phase).sin() * DRIFT,
                    y + (t / 2900.0 + phase).cos() * DRIFT,
                )
            } else {
                (x, y)
            };
            world.upsert(CellUpdate {
                id,
                kind,
                at: Circle { x, y, r },
                color: self.color(kind, owner, color),
                owner,
            });
        }
        for &FrameCell {
            kind: code,
            id,
            x,
            y,
            r,
            color,
            owner,
        } in &self.markers
        {
            if !marking {
                world.remove(id, None);
                continue;
            }
            let kind = kind_of_code(code);
            world.upsert(CellUpdate {
                id,
                kind,
                at: Circle { x, y, r },
                color: self.color(kind, owner, color),
                owner,
            });
        }
        let tabs = if self.focus == Focus::Second {
            [Tab::Second, Tab::Main]
        } else {
            [Tab::Main, Tab::Second]
        };
        for (tab, ids) in tabs.into_iter().zip(&self.frame.own) {
            for &id in ids {
                if world.cell(id).is_some() {
                    world.add_own(tab, id);
                }
            }
        }
    }
}
