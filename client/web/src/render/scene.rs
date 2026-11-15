use std::fmt::Write;

use ogar_core::world::{player_key, Aim, Cell, Circle, Kind, Player, Tab, World};

use super::backdrop::Backdrop;
use super::camera::Camera;
use super::jelly::{
    Jelly, LOD_PX as JELLY_LOD_PX, ROW as JELLY_ROW, VIEW_MARGIN as JELLY_VIEW_MARGIN,
};
use super::look::{name_for, Rings};
use super::marks::{Marks, Wanted, EAT_RATIO};
use super::motion::Motion;
use super::options::{with_alpha, Options, OverlapMarker, Rgba};
use super::overlap::{Candidate, Overlaps};
use super::primitive::{
    CardLook, Decor, Edge, FrameEdge, GlowSigma, JellyRow, Outline, Prim, Rect, Skin,
};
use super::rainbow::Rainbow;
use super::skin_atlas::SkinCache;
use super::text::{Sprite, TextAtlas, COORD_FOLLOWERS, DIGITS, SDF_PX, TIERS};

pub(super) const SECTOR_COUNT: usize = 5;
pub(super) const SECTORS: [&str; SECTOR_COUNT * SECTOR_COUNT] = [
    "A1", "A2", "A3", "A4", "A5", "B1", "B2", "B3", "B4", "B5", "C1", "C2", "C3", "C4", "C5", "D1",
    "D2", "D3", "D4", "D5", "E1", "E2", "E3", "E4", "E5",
];
const VIRUS_OUTER: f32 = 130.0 / VIRUS_MATERIAL_RADIUS;
const VIRUS_ART_BASE: f32 = 250.0;
const LABEL_FONT: f32 = 0.13;
const OWN_AIM_OWNER: u32 = 0;
const CURSOR_RED_FRAMES: u8 = 7;
pub(super) const WHITE: Rgba = [255, 255, 255, 255];
pub(super) const BIG_MASS: f32 = 20000.0;
const EAT_LIMIT_LINE: f32 = 10.0;
const EAT_LIMIT_CROSS: f32 = 100.0;
const VIRUS_MATERIAL_RADIUS: f32 = 125.0;
const FIELD_BASE: f32 = 3200.0;
const MARKER_UNIT_RADIUS: f32 = 100.0;
const SIGNAL_WAVE_MS: f64 = 1000.0;
const SIGNAL_COLOR: Rgba = [0x44, 0x55, 0x88, 255];
const VIRUS_SPLIT_MASS: f32 = 200.0;
const EJECT_MASS: f32 = 13.0;

#[derive(Debug, Clone, Copy)]
struct EatLimit {
    eater: u32,
    eaten: u32,
    limit: f32,
    can_eat: bool,
}

#[derive(Debug, Default)]
struct EatScratch {
    cells: Vec<usize>,
    limits: Vec<EatLimit>,
    shown: Vec<(u32, Circle)>,
}

pub(crate) struct Scene {
    pub under: Vec<Prim>,
    pub border: Vec<Prim>,
    pub shadows: Vec<Prim>,
    cursor_hold: u8,
    cursor_aim: Option<Aim>,
    pub jelly_rows: Vec<f32>,
    jelly: Option<Jelly>,
    jelly_row: Vec<f32>,
    pub food: Vec<Prim>,
    pub dots: Vec<Prim>,
    pub cells: Vec<Prim>,
    pub over: Vec<Prim>,
    pub overlaps: Overlaps,
    mass_spans: Vec<(u32, usize, usize)>,
    food_revision: Option<u64>,
    food_built_at: f64,
    order: Vec<Ordered>,
    eat: EatScratch,
    top: Vec<Prim>,
    own_owners: Vec<(u32, usize)>,
    pub(super) marks: Marks,
    pub(super) now: f64,
    mass_text: String,
    rainbow: Rainbow,
}

fn circle(x: f32, y: f32, r: f32) -> Circle {
    Circle { x, y, r }
}

fn disc(x: f32, y: f32, r: f32, fill: Rgba) -> Prim {
    Prim::Disc {
        at: circle(x, y, r),
        outline: Outline::Round,
        fill,
        edge: None,
    }
}

#[derive(Debug, Clone, Copy)]
struct Ordered {
    id: u32,
    at: Circle,
    index: usize,
}

fn ring(at: Circle, outline: Outline, width: f32, color: Rgba) -> Prim {
    Prim::Ring {
        at,
        outline,
        edge: Edge { width, color },
    }
}

fn push_enlarged_coord(
    under: &mut Vec<Prim>,
    atlas: &mut TextAtlas,
    text: &'static str,
    (x, y): (f32, f32),
    font: f32,
    tint: Rgba,
) -> bool {
    if !atlas.sdf_coords() {
        under.push(Prim::VectorText {
            x,
            y,
            font,
            text,
            tint,
        });
        return true;
    }
    let mut chars = text.chars();
    let (Some(first), Some(second)) = (chars.next(), chars.next()) else {
        return false;
    };
    let (a, b) = (atlas.coord_sdf(first), atlas.coord_sdf(second));
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let kern = COORD_FOLLOWERS
        .find(second)
        .map_or(0.0, |follower| a.kerns[follower]);
    let k = font / SDF_PX;
    let left = x - (a.advance + kern + b.advance) * k * 0.5;
    for (g, center) in [
        (a, left + a.advance * k * 0.5),
        (b, left + (a.advance + kern + b.advance * 0.5) * k),
    ] {
        under.push(Prim::Sdf {
            rect: Rect {
                x: center,
                y,
                half_w: g.slot.w * k * 0.5,
                half_h: g.slot.h * k * 0.5,
            },
            uv: g.slot.uv,
            page: g.slot.page,
            tint,
            texel: k,
        });
    }
    true
}

fn glyph(x: f32, y: f32, half_w: f32, half_h: f32, uv: [u16; 4], page: u32, tint: Rgba) -> Prim {
    Prim::Glyph {
        rect: Rect {
            x,
            y,
            half_w,
            half_h,
        },
        uv,
        page,
        tint,
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, color: Rgba) -> Prim {
    Prim::Rect {
        rect: Rect::spanning(x0, y0, x1, y1),
        color,
    }
}

fn segment(x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Rgba) -> Prim {
    Prim::Segment {
        from: (x0, y0),
        to: (x1, y1),
        width,
        color,
    }
}

fn gradient_ring(
    at: Circle,
    outline: Outline,
    width: f32,
    slope: f32,
    [from, to]: [Rgba; 2],
) -> Prim {
    Prim::GradientRing {
        at,
        outline,
        width,
        slope,
        from,
        to,
    }
}

fn jelly_in_view(at: Circle, cam: Camera) -> bool {
    let reach = at.r + JELLY_VIEW_MARGIN;
    (at.x - cam.x).abs() <= cam.half_view.0 + reach
        && (at.y - cam.y).abs() <= cam.half_view.1 + reach
}

pub(super) fn rgba(c: &Cell) -> Rgba {
    [c.color.0, c.color.1, c.color.2, 255]
}

pub(super) fn team_rgba(p: &Player) -> Rgba {
    [p.team_color.0, p.team_color.1, p.team_color.2, 255]
}

fn parallel_line_end(
    from: (f32, f32),
    center: (f32, f32),
    cursor: (f32, f32),
    reach: f32,
) -> (f32, f32) {
    let (dx, dy) = (cursor.0 - center.0, cursor.1 - center.1);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return from;
    }
    (from.0 + dx / len * reach, from.1 + dy / len * reach)
}

fn mass_label(out: &mut String, mass: u32, simplified: bool) {
    out.clear();
    if mass < 1000 || !simplified {
        write!(out, "{mass}").expect("write to String");
        return;
    }
    let tenths = mass / 100;
    if tenths.is_multiple_of(10) {
        write!(out, "{}K", tenths / 10).expect("write to String");
    } else {
        write!(out, "{}.{}K", tenths / 10, tenths % 10).expect("write to String");
    }
}

pub(super) fn server_mass(c: &Cell) -> u32 {
    c.to.mass().round() as u32
}

impl Scene {
    pub(crate) fn new() -> Self {
        Self {
            under: Vec::new(),
            border: Vec::new(),
            shadows: Vec::new(),
            cursor_hold: 0,
            cursor_aim: None,
            jelly_rows: Vec::new(),
            jelly: None,
            jelly_row: vec![0.0; JELLY_ROW],
            food: Vec::new(),
            dots: Vec::new(),
            cells: Vec::new(),
            over: Vec::new(),
            overlaps: Overlaps::default(),
            mass_spans: Vec::new(),
            food_revision: None,
            food_built_at: 0.0,
            order: Vec::new(),
            eat: EatScratch::default(),
            top: Vec::new(),
            own_owners: Vec::new(),
            marks: Marks::default(),
            now: 0.0,
            mass_text: String::with_capacity(8),
            rainbow: Rainbow::default(),
        }
    }

    pub(crate) fn advance_rainbow(&mut self, now: f64, o: &Options) {
        self.rainbow.advance(now, o.anim_interval, &o.rainbow_stops);
    }

    pub(crate) fn invalidate_food(&mut self) {
        self.food_revision = None;
    }

    pub(crate) fn build_under(
        &mut self,
        world: &World,
        o: &Options,
        cam: Camera,
        atlas: &mut TextAtlas,
        backdrop: &Backdrop,
    ) {
        self.under.clear();
        self.border.clear();
        if let (true, Some(size)) = (o.show_back_image, backdrop.size()) {
            self.push_backdrop(world, o, size);
        }
        if o.virus_range_hint {
            for v in world
                .cells()
                .iter()
                .filter(|c| c.kind == Kind::Virus && server_mass(c) != 50)
            {
                self.under
                    .push(disc(v.to.x, v.to.y, 880.0, o.virus_range_color));
            }
        }
        let b = world.border();
        if !b.is_valid() {
            return;
        }
        let (fw, fh) = (b.max_x - b.min_x, b.max_y - b.min_y);
        if o.show_coord {
            let alpha = f32::from(o.field_coords[3]) / 255.0;
            let label = with_alpha(
                [o.field_coords[0], o.field_coords[1], o.field_coords[2], 255],
                alpha,
            );
            let font = 0.09 * fw;
            let tier = TextAtlas::tier_for(font * cam.scale, TIERS.len() - 1);
            let enlarged = font * cam.scale > TIERS[TIERS.len() - 1];
            for (i, text) in SECTORS.iter().enumerate() {
                let (col, row) = ((i % SECTOR_COUNT) as f32, (i / SECTOR_COUNT) as f32);
                let x = b.min_x + (col + 0.5) * fw / SECTOR_COUNT as f32;
                let y = b.min_y + (row + 0.5) * fh / SECTOR_COUNT as f32;
                if enlarged
                    && push_enlarged_coord(&mut self.under, atlas, text, (x, y), font, label)
                {
                    continue;
                }
                if let Some(slot) = atlas.coord_slot(text, tier) {
                    let k = font / slot.px;
                    self.under.push(glyph(
                        x,
                        y,
                        slot.w * k * 0.5,
                        slot.h * k * 0.5,
                        slot.uv,
                        slot.page,
                        label,
                    ));
                }
            }
            let lw = 6.0 / FIELD_BASE * fw;
            let line = with_alpha(label, alpha);
            let h = lw * 0.5;
            self.under.extend([
                rect(b.min_x - h, b.min_y - h, b.min_x + h, b.max_y + h, line),
                rect(b.max_x - h, b.min_y - h, b.max_x + h, b.max_y + h, line),
                rect(b.min_x - h, b.min_y - h, b.max_x + h, b.min_y + h, line),
                rect(b.min_x - h, b.max_y - h, b.max_x + h, b.max_y + h, line),
            ]);
        }
        let t = 0.015 * fw * o.border_width;
        if t <= 0.0 {
            return;
        }
        if o.ani_border {
            let unit = fw / FIELD_BASE;
            let alpha = o.border[3];
            let colors = self.rainbow.colors().map(|c| [c[0], c[1], c[2], alpha]);
            let [from, to] = colors;
            self.under.push(Prim::Frame {
                rect: Rect::spanning(b.min_x, b.min_y, b.max_x, b.max_y),
                band: t * 0.5,
                color: from,
                edge: FrameEdge::Rainbow {
                    to,
                    sigma: 40.0 * unit,
                    reach: 140.0 * unit,
                },
            });
            return;
        }
        self.border.push(Prim::Frame {
            rect: Rect::spanning(b.min_x, b.min_y, b.max_x, b.max_y),
            band: t * 0.5,
            color: o.border,
            edge: if o.glowing_border {
                FrameEdge::Glow
            } else {
                FrameEdge::Solid
            },
        });
    }

    pub(crate) fn build_food(&mut self, world: &World, o: &Options, motion: &mut Motion) -> bool {
        let revision = world.food_revision();
        let now = motion.sample_at();
        let lerp = world.lerp_ms();
        let settled = world
            .food_moved_at()
            .is_none_or(|at| self.food_built_at >= at + lerp);
        if motion.is_segmented() && self.food_revision == Some(revision) && settled {
            return false;
        }
        self.food_revision = Some(revision);
        self.food_built_at = now;
        self.food.clear();
        if !o.show_pellet {
            return true;
        }
        for c in world.cells().iter().filter(|c| c.kind == Kind::Food) {
            let at = motion.position_of(c, lerp);
            let color = with_alpha(o.single_pellet.unwrap_or(rgba(c)), o.pellet_alpha);
            self.food.push(disc(at.x, at.y, c.from.r, color));
        }
        true
    }

    fn collect_own_owners(&mut self, world: &World) {
        self.own_owners.clear();
        for (tab_index, tab) in [Tab::Main, Tab::Second].into_iter().enumerate() {
            for c in world.own(tab).iter().filter_map(|&id| world.cell(id)) {
                if !self.own_owners.iter().any(|&(owner, _)| owner == c.owner) {
                    self.own_owners
                        .push((c.owner, tab_index * 2 + (c.owner & 1) as usize));
                }
            }
        }
    }

    pub(super) fn own_owner_slot(&self, owner: u32) -> Option<usize> {
        self.own_owners
            .iter()
            .find(|&&(o, _)| o == owner)
            .map(|&(_, slot)| slot)
    }

    fn owner_color(&self, o: &Options, cell: &Cell) -> Rgba {
        match (o.self_colors, self.own_owner_slot(cell.owner)) {
            (Some(colors), Some(slot)) => colors[slot],
            _ => rgba(cell),
        }
    }

    pub(crate) fn build_cells(
        &mut self,
        world: &World,
        o: &Options,
        cam: Camera,
        motion: &mut Motion,
        skins: &mut SkinCache,
        atlas: &mut TextAtlas,
    ) {
        let lerp = world.lerp_ms();
        self.collect_own_owners(world);
        self.order.clear();
        self.dots.clear();
        self.top.clear();
        self.now = motion.sample_at();
        let want = Wanted {
            half_split: o.show_half_split,
            mass_marker: o.show_mass_marker,
            marker_extend: o.marker_extend,
            marker_light: o.marker_light,
            split_indicator: o.show_split_indicator,
        };
        self.marks.update_marks(world, cam.aim, &want);
        for (i, c) in world.cells().iter().enumerate() {
            match c.kind {
                Kind::Food => {}
                Kind::Eject => {
                    if o.show_food {
                        let at = motion.position_of(c, lerp);
                        let color = with_alpha(self.owner_color(o, c), o.pellet_alpha);
                        self.dots.push(disc(at.x, at.y, c.from.r, color));
                    }
                }
                Kind::Player | Kind::Virus | Kind::Mother => {
                    let at = motion.position_of(c, lerp);
                    self.order.push(Ordered {
                        id: c.id,
                        at,
                        index: i,
                    });
                }
            }
        }
        self.order
            .sort_unstable_by(|a, b| a.at.r.total_cmp(&b.at.r).then(a.id.cmp(&b.id)));
        self.cells.clear();
        self.overlaps.clear();
        self.shadows.clear();
        self.mass_spans.clear();
        if !o.jelly_cells {
            self.jelly = None;
        } else if self.jelly.is_none() {
            self.jelly = Some(Jelly::new(0x9E37_79B9_7F4A_7C15));
        }
        let order = std::mem::take(&mut self.order);
        self.jelly_rows.clear();
        if let Some(jelly) = self.jelly.as_mut() {
            jelly.begin_frame();
            for &Ordered { at, index, .. } in &order {
                let cell = &world.cells()[index];
                let spiked = match cell.kind {
                    Kind::Player => false,
                    Kind::Virus | Kind::Mother => true,
                    Kind::Food | Kind::Eject => unreachable!("food and ejects are never ordered"),
                };
                if jelly_in_view(at, cam) {
                    let detailed = 2.0 * at.r * cam.css_scale >= JELLY_LOD_PX;
                    jelly.track_cell(cell.id, (at.x, at.y, at.r), spiked, detailed);
                }
            }
            jelly.retain_cells(|id| world.cell(id).is_some());
            jelly.advance_physics(self.now, world.border(), cam.css_scale, super::perf_now);
        }
        for placed in &order {
            let cell = &world.cells()[placed.index];
            match cell.kind {
                Kind::Virus | Kind::Mother => self.push_virus(o, cam, cell, placed.at, atlas),
                Kind::Player => self.push_player(world, o, cam, placed, skins, atlas),
                Kind::Food | Kind::Eject => unreachable!("food and ejects are never ordered"),
            }
        }
        self.order = order;
        self.cells.append(&mut self.top);
        let cells_end = self.cells.len();
        self.push_overlaps(o);
        if o.show_eat_limit_marker {
            self.push_eat_limits(world);
        }
        if o.show_cell_direction_marker {
            self.push_directions(world);
        }
        let playing = !world.own(Tab::Main).is_empty() || !world.own(Tab::Second).is_empty();
        let recorded = world.spectate_aim();
        let aim_line = match recorded {
            Some(a) if a.owner == OWN_AIM_OWNER => world
                .active_owner()
                .map(|owner| (owner, (a.x, a.y)))
                .filter(|_| o.show_cursor_line),
            Some(a) => o.show_spec_aim_cursor.then_some((a.owner, (a.x, a.y))),
            None if playing => world
                .active_owner()
                .zip(cam.aim)
                .filter(|_| o.show_cursor_line),
            None => None,
        };
        let mut pseudo_cursor = None;
        if let Some((owner, (ax, ay))) = aim_line {
            let parallel_center = (world.parallel_aim() && recorded.is_none())
                .then(|| {
                    let (sx, sy, n) = world
                        .cells()
                        .iter()
                        .filter(|c| {
                            c.kind == Kind::Player && player_key(c.owner) == player_key(owner)
                        })
                        .map(|c| motion.position_of(c, lerp))
                        .fold((0.0, 0.0, 0.0), |(sx, sy, n), at| {
                            (sx + at.x, sy + at.y, n + 1.0)
                        });
                    (n > 0.0).then(|| (sx / n, sy / n))
                })
                .flatten();
            let reach = 2.0 * cam.half_view.0.hypot(cam.half_view.1);
            let width = o.cursor_line_thickness * 0.2 / cam.css_scale;
            let mut drawn = 0;
            for c in world
                .cells()
                .iter()
                .filter(|c| c.owner == owner && c.kind == Kind::Player)
            {
                let at = motion.position_of(c, lerp);
                let (ex, ey) = parallel_center.map_or((ax, ay), |center| {
                    parallel_line_end((at.x, at.y), center, (ax, ay), reach)
                });
                self.cells
                    .push(segment(at.x, at.y, ex, ey, width, o.cursor_line));
                drawn += 1;
            }
            if drawn > 0 {
                pseudo_cursor = recorded;
            }
        }
        let now = motion.sample_at();
        for w in world.waves() {
            let t = ((now - w.at) / SIGNAL_WAVE_MS) as f32;
            if (0.0..1.0).contains(&t) {
                let r = 150.0 * (2.0 + 2.0 * t);
                self.cells.push(Prim::Halo {
                    at: circle(w.x, w.y, r),
                    color: with_alpha(SIGNAL_COLOR, 1.0 - t),
                });
            }
        }
        self.push_pseudo_cursor(pseudo_cursor, cam, atlas);
        self.over.clear();
        self.over.extend(self.cells.drain(cells_end..));
    }

    fn push_pseudo_cursor(&mut self, aim: Option<Aim>, cam: Camera, atlas: &mut TextAtlas) {
        if aim.is_some_and(|a| a.splitting) && aim != self.cursor_aim {
            self.cursor_hold = CURSOR_RED_FRAMES;
        }
        if aim.is_some() {
            self.cursor_aim = aim;
        }
        let red = self.cursor_hold > 0;
        self.cursor_hold = self.cursor_hold.saturating_sub(1);
        let Some(a) = aim else {
            return;
        };
        let Some(slot) = atlas.sprite_slot(if red {
            Sprite::CursorRed
        } else {
            Sprite::Cursor
        }) else {
            return;
        };
        let snap = |v: f32, center: f32, half: f32| {
            let edge = center - half;
            ((v - edge) * cam.scale).round() / cam.scale + edge
        };
        let (x, y) = (
            snap(a.x, cam.x, cam.half_view.0),
            snap(a.y, cam.y, cam.half_view.1),
        );
        let (hw, hh) = (slot.w * 0.5 / cam.css_scale, slot.h * 0.5 / cam.css_scale);
        self.cells
            .push(glyph(x + hw, y + hh, hw, hh, slot.uv, slot.page, WHITE));
    }

    fn jelly_outline(&mut self, cell: &Cell, at: Circle, cam: Camera) -> Outline {
        let Some(jelly) = self.jelly.as_mut() else {
            return Outline::Round;
        };
        if cell.kind != Kind::Player
            || !jelly_in_view(at, cam)
            || !jelly.outline_row(cell.id, self.now, &mut self.jelly_row)
        {
            return Outline::Round;
        }
        let index = JellyRow((self.jelly_rows.len() / JELLY_ROW) as u16);
        self.jelly_rows.extend_from_slice(&self.jelly_row);
        Outline::Jelly(index)
    }

    fn push_overlaps(&mut self, o: &Options) {
        self.overlaps.resolve_marks(o.overlap_marker_mask);
        self.mass_spans.sort_unstable_by_key(|s| s.0);
        for &id in self.overlaps.marked_cells() {
            let Ok(k) = self.mass_spans.binary_search_by_key(&id, |s| s.0) else {
                continue;
            };
            let (_, from, to) = self.mass_spans[k];
            for i in from..to {
                let mut p = self.cells[i];
                if let Prim::Glyph { tint, .. } = &mut p {
                    tint[3] = 179;
                }
                self.cells.push(p);
            }
        }
    }

    fn push_virus(
        &mut self,
        o: &Options,
        cam: Camera,
        cell: &Cell,
        at: Circle,
        atlas: &mut TextAtlas,
    ) {
        let (fill, stroke, labeled) = match cell.kind {
            Kind::Mother => (o.mother_fill, o.mother_stroke, false),
            _ if server_mass(cell) == 50 => (o.blue_virus_fill, o.blue_virus_stroke, false),
            _ => (o.virus_fill, o.virus_stroke, true),
        };
        if o.ani_virus {
            let alpha = stroke[3];
            let colors = self.rainbow.colors().map(|c| [c[0], c[1], c[2], alpha]);
            self.cells.extend([
                disc(at.x, at.y, at.r, fill),
                gradient_ring(
                    at,
                    Outline::Round,
                    4.0 * 0.03 * VIRUS_ART_BASE / 2.0 / VIRUS_MATERIAL_RADIUS,
                    VIRUS_MATERIAL_RADIUS / 1000.0,
                    colors,
                ),
            ]);
        } else {
            self.cells.push(if o.simple_virus {
                Prim::Disc {
                    at: circle(at.x, at.y, at.r * VIRUS_OUTER),
                    outline: Outline::Round,
                    fill,
                    edge: Some(Edge {
                        width: (VIRUS_OUTER - 115.0 / VIRUS_MATERIAL_RADIUS) / VIRUS_OUTER,
                        color: stroke,
                    }),
                }
            } else {
                Prim::Star { at, fill, stroke }
            });
        }
        let lod = LABEL_FONT * 2.0 * at.r * o.mass_zoom * cam.css_scale;
        if !labeled || !o.show_mass || lod < o.min_text_px {
            return;
        }
        let mass = server_mass(cell);
        let box_font = if o.virus_split_hint {
            32.0
        } else {
            LABEL_FONT * o.card_size
        };
        let (value, scale, divisor) = if o.virus_split_hint {
            let splits = ((VIRUS_SPLIT_MASS - mass as f32) / EJECT_MASS).ceil();
            (splits as i64, 4.0, 2.2)
        } else {
            (i64::from(mass), 1.0, 2.5)
        };
        let font = box_font * 2.0 * at.r / VIRUS_ART_BASE * scale * o.mass_zoom;
        self.mass_text.clear();
        write!(self.mass_text, "{value}").expect("write to String");
        let tier = TextAtlas::tier_for(font * cam.scale, o.text_max_tier);
        self.push_digits(
            (at.x, at.y),
            font,
            divisor,
            with_alpha(WHITE, o.label_alpha),
            tier,
            atlas,
        );
    }

    fn push_backdrop(&mut self, world: &World, o: &Options, (iw, ih): (f32, f32)) {
        let b = world.border();
        if !b.is_valid() {
            return;
        }
        let fw = b.max_x - b.min_x;
        let scale = fw / iw.min(ih);
        let (cx, cy) = b.center();
        let (hw, hh) = (iw * scale / 2.0, ih * scale / 2.0);
        let (x0, y0, x1, y1) = (cx - hw, cy - hh, cx + hw, cy + hh);
        let (cx0, cy0, cx1, cy1) = (
            x0.max(b.min_x),
            y0.max(b.min_y),
            x1.min(b.max_x),
            y1.min(b.max_y),
        );
        let n =
            |v: f32, lo: f32, hi: f32| ((v - lo) / (hi - lo) * f32::from(u16::MAX)).round() as u16;
        let uv = [
            n(cx0, x0, x1),
            n(cy0, y0, y1),
            n(cx1, x0, x1),
            n(cy1, y0, y1),
        ];
        self.under.push(Prim::Image {
            rect: Rect::spanning(cx0, cy0, cx1, cy1),
            uv,
            opacity: (255.0 * o.back_image_alpha).round() as u8,
        });
    }

    fn shown_circle_of(&self, id: u32) -> Option<Circle> {
        let shown = &self.eat.shown;
        shown
            .binary_search_by_key(&id, |&(shown_id, _)| shown_id)
            .ok()
            .map(|i| shown[i].1)
    }

    fn push_eat_limits(&mut self, world: &World) {
        let all = world.cells();
        let eat = &mut self.eat;
        eat.cells.clear();
        eat.cells.extend(
            all.iter()
                .enumerate()
                .filter(|(_, c)| c.kind == Kind::Player)
                .map(|(i, _)| i),
        );
        eat.cells.sort_unstable_by(|&a, &b| {
            (all[a].to.x - all[a].to.r).total_cmp(&(all[b].to.x - all[b].to.r))
        });
        eat.limits.clear();
        for (i, &ai) in eat.cells.iter().enumerate() {
            let a = &all[ai];
            let right = a.to.x + a.to.r;
            for &bi in &eat.cells[i + 1..] {
                let b = &all[bi];
                if b.to.x - b.to.r >= right {
                    break;
                }
                if a.owner == b.owner {
                    continue;
                }
                let dist = ((a.to.x - b.to.x).powi(2) + (a.to.y - b.to.y).powi(2)).sqrt();
                if dist >= a.to.r.max(b.to.r) {
                    continue;
                }
                let (eater, eaten) = if a.to.r > b.to.r { (a, b) } else { (b, a) };
                eat.limits.push(EatLimit {
                    eater: eater.id,
                    eaten: eaten.id,
                    limit: eater.to.r - 0.41 * eaten.to.r,
                    can_eat: server_mass(eater) as f32 > EAT_RATIO * server_mass(eaten) as f32,
                });
            }
        }
        eat.limits
            .sort_by(|x, y| x.eater.cmp(&y.eater).then(y.limit.total_cmp(&x.limit)));
        eat.limits.dedup_by_key(|l| l.eater);
        eat.shown.clear();
        eat.shown.extend(self.order.iter().map(|o| (o.id, o.at)));
        eat.shown.sort_unstable_by_key(|&(id, _)| id);
        let limits = std::mem::take(&mut self.eat.limits);
        for &EatLimit {
            eater,
            eaten,
            limit,
            can_eat,
        } in &limits
        {
            let (Some(e), Some(n)) = (self.shown_circle_of(eater), self.shown_circle_of(eaten))
            else {
                continue;
            };
            let color = if can_eat {
                [0x00, 0xFF, 0xFF, 255]
            } else {
                [0xAA, 0xAA, 0xAA, 255]
            };
            for c in [e, n] {
                self.cells.push(rect(
                    c.x - EAT_LIMIT_CROSS,
                    c.y - EAT_LIMIT_LINE / 2.0,
                    c.x + EAT_LIMIT_CROSS,
                    c.y + EAT_LIMIT_LINE / 2.0,
                    color,
                ));
                self.cells.push(rect(
                    c.x - EAT_LIMIT_LINE / 2.0,
                    c.y - EAT_LIMIT_CROSS,
                    c.x + EAT_LIMIT_LINE / 2.0,
                    c.y + EAT_LIMIT_CROSS,
                    color,
                ));
            }
            let outer = limit + EAT_LIMIT_LINE / 2.0;
            self.cells.push(ring(
                circle(e.x, e.y, outer),
                Outline::Round,
                EAT_LIMIT_LINE / outer,
                color,
            ));
        }
        self.eat.limits = limits;
    }

    fn push_directions(&mut self, world: &World) {
        let order = std::mem::take(&mut self.order);
        for &Ordered { at, index, .. } in &order {
            let c = &world.cells()[index];
            if c.kind != Kind::Player {
                continue;
            }
            let (vx, vy) = c.velocity;
            if vx == 0.0 && vy == 0.0 {
                continue;
            }
            let angle = vy.atan2(vx);
            let (cos, sin) = (angle.cos(), angle.sin());
            let (near, far) = (at.r * 0.8, at.r * 1.05);
            let width = (0.05 * c.to.r).max(15.0);
            let color = world.player(c.owner).map_or(WHITE, team_rgba);
            self.cells.push(segment(
                c.to.x + cos * near,
                c.to.y + sin * near,
                c.to.x + cos * far,
                c.to.y + sin * far,
                width,
                color,
            ));
        }
        self.order = order;
    }

    fn push_rings(
        &mut self,
        o: &Options,
        cell: &Cell,
        at: Circle,
        outline: Outline,
        rings: &Rings,
    ) {
        let s = at.r / MARKER_UNIT_RADIUS;
        let marker_alpha = o.marker_alpha;
        if let Some(c) = rings.edge {
            let line = o.marker_thickness * s;
            let c = with_alpha(c, marker_alpha);
            self.cells.push(ring(at, outline, line / at.r, c));
            if rings.slash {
                let d = std::f32::consts::FRAC_1_SQRT_2 * at.r;
                self.cells
                    .push(segment(at.x + d, at.y - d, at.x - d, at.y + d, line, c));
            }
        }
        if rings.rainbow {
            let width = o.marker_thickness / MARKER_UNIT_RADIUS;
            let slope = 0.5 / (1.0 - o.marker_thickness / 200.0);
            self.cells.push(gradient_ring(
                at,
                outline,
                width,
                slope,
                self.rainbow.colors(),
            ));
        }
        if let Some(c) = rings.second {
            let line = if server_mass(cell) as f32 > BIG_MASS {
                8.75
            } else {
                6.25
            } * s;
            self.cells
                .push(ring(at, outline, line / at.r, with_alpha(c, marker_alpha)));
        }
        if let Some(c) = rings.outer {
            let line = 5.0 * s;
            let outer = at.r + 10.0 * s + line / 2.0;
            self.cells.push(ring(
                circle(at.x, at.y, outer),
                Outline::Round,
                line / outer,
                with_alpha(c, marker_alpha),
            ));
        }
        if rings.arrow {
            self.cells.push(Prim::Arrow {
                x: at.x,
                y: at.y - at.r - 75.0,
                half: 300.0 / 2.0,
                color: WHITE,
            });
        }
    }

    fn push_player(
        &mut self,
        world: &World,
        o: &Options,
        cam: Camera,
        placed: &Ordered,
        skins: &mut SkinCache,
        atlas: &mut TextAtlas,
    ) {
        let cell = &world.cells()[placed.index];
        let at = placed.at;
        let outline = self.jelly_outline(cell, at, cam);
        let mut mass_start = None;
        let own = self.own_owner_slot(cell.owner).is_some();
        if own && o.invisible_self {
            return;
        }
        if o.cell_shadow {
            self.shadows.push(Prim::Shadow {
                at,
                outline,
                color: with_alpha([0, 0, 0, 255], o.cell_shadow_alpha),
            });
        }
        let player = world.player(cell.owner);
        let base = self.owner_color(o, cell);
        let marking = match o.overlap_marker {
            OverlapMarker::Off => false,
            OverlapMarker::Auto => server_mass(cell) as f32 >= 10_000.0,
            OverlapMarker::On => true,
        };
        if marking {
            self.overlaps.push_candidate(Candidate {
                id: cell.id,
                owner: cell.owner,
                at,
                color: base,
            });
        }
        let rings = self.rings_for(world, o, cell, own);
        let base = rings.body.unwrap_or(base);
        let mut card = CardLook {
            body: [base[0], base[1], base[2]],
            alpha: f32::from(base[3]) / 255.0 * o.cell_alpha,
            skin: None,
            decor: rings
                .order3
                .filter(|t| t[3] > 0)
                .map_or(Decor::None, Decor::Tint),
        };
        let look = player.map(|p| self.skin_look(world, o, cell, p));
        if let (Some(look), Some(p)) = (look, player) {
            if look.enemy_overlay {
                card.decor = Decor::Enemy(with_alpha(team_rgba(p), 0.8));
            }
        }
        if let Some(look) = look.filter(|l| l.alpha > 0.0) {
            let url = world.string(look.key);
            let layer = if url.is_empty() {
                None
            } else {
                skins.layer_for_url(url)
            };
            if let Some(layer) = layer {
                card.skin = Some(Skin {
                    layer,
                    alpha: look.alpha,
                });
            }
        }
        let glow = o.glowing_cells.then(|| {
            GlowSigma::new(if card.skin.is_some() {
                1.0 / 22.0
            } else {
                0.25
            })
        });
        self.cells.push(Prim::Card {
            at,
            outline,
            look: card,
            glow,
        });
        if let (true, Some(p), Some(label)) =
            (o.show_circular_name, player, name_for(o, player, own))
        {
            let font = 0.2 * at.r;
            if own || font * cam.css_scale >= o.min_text_px {
                let tier = TextAtlas::tier_for(font * cam.scale, o.text_max_tier);
                if let Some(slot) =
                    atlas.circular_name((world.id(), label.name), || label.text(world), tier)
                {
                    self.cells.push(glyph(
                        at.x,
                        at.y,
                        at.r,
                        at.r,
                        slot.uv,
                        slot.page,
                        with_alpha(team_rgba(p), o.cell_alpha),
                    ));
                }
            }
        }
        self.push_rings(o, cell, at, outline, &rings);

        let lod =
            |zoom: f32| own || LABEL_FONT * 2.0 * at.r * zoom * cam.css_scale >= o.min_text_px;
        let mass_visible = lod(o.mass_zoom);
        let label_alpha = o.label_alpha;
        let name = name_for(o, player, own).filter(|_| !o.show_circular_name);
        if let Some(label) = name.filter(|_| mass_visible && lod(o.name_zoom)) {
            let font = LABEL_FONT * 2.0 * at.r * o.name_zoom;
            let tier = TextAtlas::tier_for(font * cam.scale, o.text_max_tier);
            if let Some(slot) =
                atlas.name_slot((world.id(), label.name), || label.text(world), tier)
            {
                let k = font / slot.px;
                self.cells.push(glyph(
                    at.x,
                    at.y,
                    slot.w * k * 0.5,
                    slot.h * k * 0.5,
                    slot.uv,
                    slot.page,
                    with_alpha(label.color, label_alpha),
                ));
            }
        }
        if o.show_mass && mass_visible {
            let font = LABEL_FONT * 2.0 * at.r * o.mass_zoom;
            let y = if name.is_some() {
                at.y + 0.2 * 2.0 * at.r
            } else {
                at.y
            };
            mass_label(&mut self.mass_text, server_mass(cell), o.simplified_mass);
            let from = self.cells.len();
            mass_start = Some(from);
            self.push_digits(
                (at.x, y),
                font,
                2.0,
                with_alpha(player.map_or(WHITE, team_rgba), label_alpha),
                TextAtlas::tier_for(font * cam.scale, o.text_max_tier),
                atlas,
            );
            if marking {
                self.mass_spans.push((cell.id, from, self.cells.len()));
            }
        }
        let on_top = o.overlap_marker == OverlapMarker::Off
            && server_mass(cell) as f32 > o.mass_overlap_size;
        if let (true, Some(from)) = (on_top, mass_start) {
            self.top.extend(self.cells.drain(from..));
        }
    }

    fn push_digits(
        &mut self,
        (x, y): (f32, f32),
        font: f32,
        top_divisor: f32,
        color: Rgba,
        tier: usize,
        atlas: &mut TextAtlas,
    ) {
        let Some(digits) = atlas.digit_slots(tier) else {
            return;
        };
        let k = font / digits[0].slot.px;
        let glyphs = self.mass_text.chars().filter_map(|ch| DIGITS.find(ch));
        let width: f32 = glyphs.clone().map(|i| digits[i].advance).sum::<f32>() * k;
        let height = digits[0].slot.h * k;
        let center_y = y - height / top_divisor + height / 2.0;
        let mut cursor = x - width * 0.5;
        for i in glyphs {
            let d = digits[i];
            let adv = d.advance * k;
            self.cells.push(glyph(
                cursor + adv * 0.5,
                center_y,
                d.slot.w * k * 0.5,
                d.slot.h * k * 0.5,
                d.slot.uv,
                d.slot.page,
                color,
            ));
            cursor += adv;
        }
    }
}
