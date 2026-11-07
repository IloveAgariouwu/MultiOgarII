use std::f32::consts::{SQRT_2, TAU};
use std::fmt::Write;

use bytemuck::{Pod, Zeroable};
use ogar_core::world::Circle;

use super::jelly::{CORE as JELLY_CORE, SAMPLES as JELLY_SAMPLES};
use super::options::Rgba;
use super::text::SDF_SPREAD;

pub(crate) const GLOW_REACH: f32 = 3.0;
pub(crate) const GLOW_UNITS: f32 = 1000.0;
pub(crate) const HALO_INNER: f32 = 0.75;
pub(crate) const FRAME_GLOW_SIGMA: f32 = 1.125;
pub(crate) const STAR_EXTENT: f32 = 1.12;
pub(crate) const STAR_OUTER: f32 = 1.08;
pub(crate) const STAR_INNER: f32 = 0.92;
pub(crate) const STAR_LINE: f32 = 0.065;
pub(crate) const STAR_POINTS: usize = 24;
pub(crate) const ARROW_TIP: f32 = 0.2;
pub(crate) const ARROW_BASE: f32 = -0.7;
pub(crate) const ARROW_HALF_WIDTH: f32 = 0.7;
pub(crate) const SHADOW_REACH: f32 = 1.15;
pub(crate) const SHADOW_FEATHER: f32 = 0.0115;
pub(crate) const ENEMY_LINE: f32 = 0.12;
pub(crate) const ENEMY_RING_INSET: f32 = 0.14;
pub(crate) const ENEMY_DIM: f32 = 0.3;
pub(crate) const OVERLAP_RIM: f32 = 0.984;
pub(crate) const OVERLAP_GLOW_SPREAD: f32 = 0.032768;
const GLOW_CUTOFF: f32 = 1.0 / 512.0;
const AA_MARGIN_PX: f32 = 1.5;
const MIN_RADIUS_PX: f32 = 0.001;

const KIND_MASK: u32 = 0x1F;
const FLAG_SHIFT: u32 = 5;
const FLAG_MASK: u32 = 0x7;
const GLOW_SHIFT: u32 = 8;
const GLOW_MASK: u32 = 0xFF;
const LAYER_SHIFT: u32 = 16;
const NO_JELLY: u16 = u16::MAX;
const CARD_SKINNED: u32 = 1;
const CARD_TINT: u32 = 2;
const CARD_ENEMY: u32 = 4;
const FRAME_GLOW: u32 = 1;
const FRAME_RAINBOW: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
enum Kind {
    Disc = 0,
    Glyph = 1,
    Rect = 2,
    Segment = 3,
    Star = 4,
    Halo = 5,
    Frame = 6,
    Arrow = 7,
    Image = 8,
    Gradient = 9,
    Shadow = 10,
    Card = 11,
    Sdf = 12,
}

const KINDS: [(&str, Kind); 13] = [
    ("KIND_DISC", Kind::Disc),
    ("KIND_GLYPH", Kind::Glyph),
    ("KIND_RECT", Kind::Rect),
    ("KIND_SEGMENT", Kind::Segment),
    ("KIND_STAR", Kind::Star),
    ("KIND_HALO", Kind::Halo),
    ("KIND_FRAME", Kind::Frame),
    ("KIND_ARROW", Kind::Arrow),
    ("KIND_IMAGE", Kind::Image),
    ("KIND_GRADIENT", Kind::Gradient),
    ("KIND_SHADOW", Kind::Shadow),
    ("KIND_CARD", Kind::Card),
    ("KIND_SDF", Kind::Sdf),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JellyRow(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outline {
    Round,
    Jelly(JellyRow),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlowSigma(u8);

impl GlowSigma {
    pub(crate) fn new(sigma: f32) -> Self {
        Self((sigma * GLOW_UNITS).round().clamp(0.0, f32::from(u8::MAX)) as u8)
    }

    pub(crate) fn sigma(self) -> f32 {
        f32::from(self.0) / GLOW_UNITS
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Edge {
    pub width: f32,
    pub color: Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub half_w: f32,
    pub half_h: f32,
}

impl Rect {
    pub(crate) fn spanning(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            x: (x0 + x1) * 0.5,
            y: (y0 + y1) * 0.5,
            half_w: (x1 - x0) * 0.5,
            half_h: (y1 - y0) * 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Skin {
    pub layer: u32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Decor {
    None,
    Tint(Rgba),
    Enemy(Rgba),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardLook {
    pub body: [u8; 3],
    pub alpha: f32,
    pub skin: Option<Skin>,
    pub decor: Decor,
}

impl CardLook {
    pub(crate) fn flat(&self) -> Option<Rgba> {
        let (tint, beta) = match (self.skin, self.decor) {
            (Some(_), _) | (None, Decor::Enemy(_)) => return None,
            (None, Decor::None) => ([0; 4], 0.0),
            (None, Decor::Tint(t)) => (t, f32::from(t[3]) / 255.0),
        };
        let a = self.alpha;
        let under = a * (1.0 - beta);
        let cover = a * beta + under;
        let channel = |k: usize| {
            let premul = a * beta * f32::from(tint[k]) + under * f32::from(self.body[k]);
            (premul / cover.max(f32::EPSILON)).round() as u8
        };
        Some([
            channel(0),
            channel(1),
            channel(2),
            (cover * 255.0).round() as u8,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrameEdge {
    Solid,
    Glow,
    Rainbow { to: Rgba, sigma: f32, reach: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Prim {
    Disc {
        at: Circle,
        outline: Outline,
        fill: Rgba,
        edge: Option<Edge>,
    },
    Ring {
        at: Circle,
        outline: Outline,
        edge: Edge,
    },
    Card {
        at: Circle,
        outline: Outline,
        look: CardLook,
        glow: Option<GlowSigma>,
    },
    Shadow {
        at: Circle,
        outline: Outline,
        color: Rgba,
    },
    Halo {
        at: Circle,
        color: Rgba,
    },
    GradientRing {
        at: Circle,
        outline: Outline,
        width: f32,
        slope: f32,
        from: Rgba,
        to: Rgba,
    },
    Star {
        at: Circle,
        fill: Rgba,
        stroke: Rgba,
    },
    Glyph {
        rect: Rect,
        uv: [u16; 4],
        page: u32,
        tint: Rgba,
    },
    Sdf {
        rect: Rect,
        uv: [u16; 4],
        page: u32,
        tint: Rgba,
        texel: f32,
    },
    VectorText {
        x: f32,
        y: f32,
        font: f32,
        text: &'static str,
        tint: Rgba,
    },
    Rect {
        rect: Rect,
        color: Rgba,
    },
    Segment {
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        color: Rgba,
    },
    Arrow {
        x: f32,
        y: f32,
        half: f32,
        color: Rgba,
    },
    Image {
        rect: Rect,
        uv: [u16; 4],
        opacity: u8,
    },
    Frame {
        rect: Rect,
        band: f32,
        color: Rgba,
        edge: FrameEdge,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Instance {
    pub geom: [f32; 4],
    pub aux: [u16; 4],
    pub color0: Rgba,
    pub color1: Rgba,
    pub scalar: f32,
    pub head: u32,
}

pub(crate) const INSTANCE_BYTES: u32 = std::mem::size_of::<Instance>() as u32;
const _: () = assert!(INSTANCE_BYTES == 40, "instance stride");

fn head(kind: Kind, flags: u32, glow: Option<GlowSigma>, layer: u32) -> u32 {
    kind as u32
        | (flags << FLAG_SHIFT)
        | (u32::from(glow.map_or(0, |g| g.0)) << GLOW_SHIFT)
        | (layer << LAYER_SHIFT)
}

fn jelly(outline: Outline) -> u16 {
    match outline {
        Outline::Round => NO_JELLY,
        Outline::Jelly(JellyRow(row)) => row,
    }
}

fn premul(c: Rgba) -> Rgba {
    let a = u16::from(c[3]);
    let scale = |v: u8| ((u16::from(v) * a + 127) / 255) as u8;
    [scale(c[0]), scale(c[1]), scale(c[2]), c[3]]
}

fn unorm16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16
}

fn round(at: Circle, outline: Outline, aux0: u16) -> ([f32; 4], [u16; 4]) {
    ([at.x, at.y, at.r, at.r], [aux0, aux0, aux0, jelly(outline)])
}

fn rect_geom(r: Rect) -> [f32; 4] {
    [r.x, r.y, r.half_w, r.half_h]
}

pub(crate) fn pack(p: &Prim) -> Instance {
    let blank = Instance {
        geom: [0.0; 4],
        aux: [0; 4],
        color0: [0; 4],
        color1: [0; 4],
        scalar: 0.0,
        head: 0,
    };
    match *p {
        Prim::Disc {
            at,
            outline,
            fill,
            edge,
        } => {
            let (geom, aux) = round(at, outline, 0);
            Instance {
                geom,
                aux,
                color0: premul(fill),
                color1: edge.map_or([0; 4], |e| premul(e.color)),
                scalar: edge.map_or(0.0, |e| e.width),
                head: head(Kind::Disc, 0, None, 0),
            }
        }
        Prim::Ring { at, outline, edge } => {
            let (geom, aux) = round(at, outline, 0);
            Instance {
                geom,
                aux,
                color1: premul(edge.color),
                scalar: edge.width,
                head: head(Kind::Disc, 0, None, 0),
                ..blank
            }
        }
        Prim::Card {
            at,
            outline,
            look,
            glow,
        } => {
            let (geom, _) = round(at, outline, 0);
            let aux = [
                look.skin.map_or(0, |s| unorm16(s.alpha)),
                unorm16(look.alpha),
                0,
                jelly(outline),
            ];
            let (decor_flag, decor) = match look.decor {
                Decor::None => (0, [0; 4]),
                Decor::Tint(c) => (CARD_TINT, premul(c)),
                Decor::Enemy(c) => (CARD_ENEMY, c),
            };
            let (skin_flag, layer) = look.skin.map_or((0, 0), |s| (CARD_SKINNED, s.layer));
            let [r, g, b] = look.body;
            Instance {
                geom,
                aux,
                color0: [r, g, b, u8::MAX],
                color1: decor,
                scalar: 0.0,
                head: head(Kind::Card, skin_flag | decor_flag, glow, layer),
            }
        }
        Prim::Shadow { at, outline, color } => {
            let (geom, aux) = round(at, outline, 0);
            Instance {
                geom,
                aux,
                color0: premul(color),
                head: head(Kind::Shadow, 0, None, 0),
                ..blank
            }
        }
        Prim::Halo { at, color } => {
            let (geom, aux) = round(at, Outline::Round, 0);
            Instance {
                geom,
                aux,
                color0: premul(color),
                head: head(Kind::Halo, 0, None, 0),
                ..blank
            }
        }
        Prim::GradientRing {
            at,
            outline,
            width,
            slope,
            from,
            to,
        } => {
            let (geom, aux) = round(at, outline, unorm16(slope));
            Instance {
                geom,
                aux,
                color0: premul(from),
                color1: premul(to),
                scalar: width,
                head: head(Kind::Gradient, 0, None, 0),
            }
        }
        Prim::Star { at, fill, stroke } => Instance {
            geom: [at.x, at.y, at.r * STAR_EXTENT, at.r * STAR_EXTENT],
            color0: premul(fill),
            color1: premul(stroke),
            head: head(Kind::Star, 0, None, 0),
            ..blank
        },
        Prim::Glyph {
            rect,
            uv,
            page,
            tint,
        } => Instance {
            geom: rect_geom(rect),
            aux: uv,
            color0: premul(tint),
            head: head(Kind::Glyph, 0, None, page),
            ..blank
        },
        Prim::Sdf {
            rect,
            uv,
            page,
            tint,
            texel,
        } => Instance {
            geom: rect_geom(rect),
            aux: uv,
            color0: premul(tint),
            scalar: texel,
            head: head(Kind::Sdf, 0, None, page),
            ..blank
        },
        Prim::VectorText { .. } => unreachable!("only the 2d backend asks for vector text"),
        Prim::Rect { rect, color } => Instance {
            geom: rect_geom(rect),
            color0: premul(color),
            head: head(Kind::Rect, 0, None, 0),
            ..blank
        },
        Prim::Segment {
            from,
            to,
            width,
            color,
        } => Instance {
            geom: [from.0, from.1, to.0, to.1],
            color0: premul(color),
            scalar: width * 0.5,
            head: head(Kind::Segment, 0, None, 0),
            ..blank
        },
        Prim::Arrow { x, y, half, color } => Instance {
            geom: [x, y, half, half],
            color0: premul(color),
            head: head(Kind::Arrow, 0, None, 0),
            ..blank
        },
        Prim::Image { rect, uv, opacity } => Instance {
            geom: rect_geom(rect),
            aux: uv,
            color0: [opacity; 4],
            head: head(Kind::Image, 0, None, 0),
            ..blank
        },
        Prim::Frame {
            rect,
            band,
            color,
            edge,
        } => {
            let half = (rect.half_w.min(rect.half_h)).max(f32::EPSILON);
            let (flags, aux, color1) = match edge {
                FrameEdge::Solid => (0, [0; 4], [0; 4]),
                FrameEdge::Glow => (FRAME_GLOW, [0; 4], [0; 4]),
                FrameEdge::Rainbow { to, sigma, reach } => (
                    FRAME_RAINBOW,
                    [unorm16(sigma / half), unorm16(reach / half), 0, 0],
                    premul(to),
                ),
            };
            Instance {
                geom: rect_geom(rect),
                aux,
                color0: premul(color),
                color1,
                scalar: band,
                head: head(Kind::Frame, flags, None, 0),
            }
        }
    }
}

pub(crate) fn pack_into(prims: &[Prim], out: &mut Vec<Instance>) {
    out.clear();
    out.extend(prims.iter().map(pack));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShaderLang {
    Wgsl,
    Glsl,
}

#[derive(Debug, Clone, Copy)]
enum ShaderConst {
    F32(f32),
    U32(u32),
    I32(i32),
}

fn shader_constants() -> Vec<(&'static str, ShaderConst)> {
    use ShaderConst::{F32, I32, U32};
    let mut table = vec![
        ("GLOW_REACH", F32(GLOW_REACH)),
        ("GLOW_UNITS", F32(GLOW_UNITS)),
        ("GLOW_CUTOFF", F32(GLOW_CUTOFF)),
        ("HALO_INNER", F32(HALO_INNER)),
        ("FRAME_GLOW_SIGMA", F32(FRAME_GLOW_SIGMA)),
        ("STAR_EXTENT", F32(STAR_EXTENT)),
        ("STAR_OUTER", F32(STAR_OUTER)),
        ("STAR_INNER", F32(STAR_INNER)),
        ("STAR_HALF_LINE", F32(STAR_LINE * 0.5)),
        ("STAR_SEGMENT", F32(TAU / STAR_POINTS as f32)),
        ("ARROW_TIP", F32(ARROW_TIP)),
        ("ARROW_BASE", F32(ARROW_BASE)),
        ("ARROW_HALF_WIDTH", F32(ARROW_HALF_WIDTH)),
        ("SHADOW_REACH", F32(SHADOW_REACH)),
        ("SHADOW_FEATHER", F32(SHADOW_FEATHER)),
        ("ENEMY_LINE", F32(ENEMY_LINE)),
        ("ENEMY_RING_INSET", F32(ENEMY_RING_INSET)),
        ("ENEMY_DIM", F32(ENEMY_DIM)),
        ("SLASH_HALF", F32(1.0 - ENEMY_RING_INSET * SQRT_2)),
        ("OVERLAP_RIM", F32(OVERLAP_RIM)),
        ("OVERLAP_GLOW_SPREAD", F32(OVERLAP_GLOW_SPREAD)),
        ("AA_MARGIN_PX", F32(AA_MARGIN_PX)),
        ("MIN_RADIUS_PX", F32(MIN_RADIUS_PX)),
        ("TAU", F32(TAU)),
        ("SDF_SPREAD", F32(SDF_SPREAD)),
        ("JELLY_CORE", F32(JELLY_CORE)),
        ("JELLY_SAMPLES", I32(JELLY_SAMPLES as i32)),
        ("NO_JELLY", U32(u32::from(NO_JELLY))),
        ("KIND_MASK", U32(KIND_MASK)),
        ("FLAG_SHIFT", U32(FLAG_SHIFT)),
        ("FLAG_MASK", U32(FLAG_MASK)),
        ("GLOW_SHIFT", U32(GLOW_SHIFT)),
        ("GLOW_MASK", U32(GLOW_MASK)),
        ("LAYER_SHIFT", U32(LAYER_SHIFT)),
        ("CARD_SKINNED", U32(CARD_SKINNED)),
        ("CARD_TINT", U32(CARD_TINT)),
        ("CARD_ENEMY", U32(CARD_ENEMY)),
        ("FRAME_GLOW", U32(FRAME_GLOW)),
        ("FRAME_RAINBOW", U32(FRAME_RAINBOW)),
    ];
    table.extend(KINDS.iter().map(|&(name, kind)| (name, U32(kind as u32))));
    table
}

pub(crate) fn shader_source(lang: ShaderLang, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 2048);
    let rest = match lang {
        ShaderLang::Wgsl => body,
        ShaderLang::Glsl => {
            let split = body
                .match_indices('\n')
                .take_while(|&(at, _)| {
                    let line = body[..at].lines().last().unwrap_or_default();
                    line.starts_with("#version") || line.starts_with("precision")
                })
                .last()
                .map_or(0, |(at, _)| at + 1);
            out.push_str(&body[..split]);
            &body[split..]
        }
    };
    for (name, value) in shader_constants() {
        match (lang, value) {
            (ShaderLang::Wgsl, ShaderConst::F32(v)) => writeln!(out, "const {name}: f32 = {v:?};"),
            (ShaderLang::Wgsl, ShaderConst::U32(v)) => writeln!(out, "const {name}: u32 = {v}u;"),
            (ShaderLang::Wgsl, ShaderConst::I32(v)) => writeln!(out, "const {name}: i32 = {v};"),
            (ShaderLang::Glsl, ShaderConst::F32(v)) => writeln!(out, "const float {name} = {v:?};"),
            (ShaderLang::Glsl, ShaderConst::U32(v)) => writeln!(out, "const uint {name} = {v}u;"),
            (ShaderLang::Glsl, ShaderConst::I32(v)) => writeln!(out, "const int {name} = {v};"),
        }
        .expect("write to String");
    }
    out.push_str(rest);
    out
}
