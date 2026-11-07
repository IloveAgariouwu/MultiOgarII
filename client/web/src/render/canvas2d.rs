use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::hash::Hash;

use ogar_core::world::Circle;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageBitmap, ImageData, OffscreenCanvas,
    OffscreenCanvasRenderingContext2d,
};

use super::backdrop::Backdrop;
use super::jelly::{self, ROW as JELLY_ROW, SAMPLES as JELLY_SAMPLES};
use super::options::Rgba;
use super::overlap::OverlapMark;
use super::primitive::{
    CardLook, Decor, Edge, FrameEdge, GlowSigma, JellyRow, Outline, Prim, Rect, ARROW_BASE,
    ARROW_HALF_WIDTH, ARROW_TIP, ENEMY_DIM, ENEMY_LINE, ENEMY_RING_INSET, FRAME_GLOW_SIGMA,
    GLOW_REACH, HALO_INNER, OVERLAP_GLOW_SPREAD, OVERLAP_RIM, SHADOW_FEATHER, SHADOW_REACH,
    STAR_EXTENT, STAR_INNER, STAR_LINE, STAR_OUTER, STAR_POINTS,
};
use super::skin_atlas::{mip_sides, SkinCache};
use super::text::{TextAtlas, ATLAS_SIDE, SDF_PX};
use super::{FrameView, Layers};
use glyph_sheet::{GlyphSheet, TintKey};

mod glyph_sheet;

#[wasm_bindgen(module = "/src/render/canvas2d_path.js")]
extern "C" {
    #[wasm_bindgen(js_name = tracePolygon)]
    fn trace_polygon(ctx: &CanvasRenderingContext2d, points: &[f32]);
}

const JELLY_CHORD_ERROR_PX: f32 = 0.25;
const FOOD_SQUARE_MAX_RADIUS_PX: f64 = 1.5;
const GLOW_HOLE_INSET_TEXELS: f64 = 1.5;
const JELLY_MIN_SAMPLES: usize = 16;
const OVERLAP_STOPS: usize = 16;
const SHADOW_SIDE: u32 = 512;
const FRAME_GLOW_STOPS: usize = 12;
const JELLY_REACH: f32 = 0.5;
const BAKED_BUDGET: usize = 32 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Cut {
    Disc,
    Bleed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CardKey {
    skin: Option<(u32, u8)>,
    body: [u8; 3],
    decor: Decor,
    side: u32,
    cut: Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BakedKey {
    Card(CardKey),
    Mark {
        color: Rgba,
        side: u32,
    },
    Glow {
        rgb: [u8; 3],
        sigma: GlowSigma,
        side: u32,
    },
}

impl CardKey {
    fn canvas_side(&self) -> u32 {
        match self.cut {
            Cut::Disc => self.side,
            Cut::Bleed => (self.side as f32 * (1.0 + JELLY_REACH)).ceil() as u32,
        }
    }
}

fn pick_mip_side(ladder: &[u32], diameter: f32) -> (u32, bool) {
    let top = ladder[0];
    match ladder.iter().rev().find(|&&s| s as f32 >= diameter) {
        Some(&side) => (side, true),
        None => (top, false),
    }
}

struct Cached<V> {
    value: V,
    bytes: usize,
    used: u64,
}

struct ByteLru<K, V> {
    entries: HashMap<K, Cached<V>>,
    bytes: usize,
    frame: u64,
    stale: Vec<(u64, K)>,
}

impl<K: Copy + Eq + Hash, V> ByteLru<K, V> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            frame: 0,
            stale: Vec::new(),
        }
    }

    fn next_frame(&mut self) {
        self.frame += 1;
    }

    fn get_or_try_insert<E>(
        &mut self,
        key: K,
        make: impl FnOnce() -> Result<(V, usize), E>,
    ) -> Result<&V, E> {
        let entry = match self.entries.entry(key) {
            Entry::Occupied(slot) => slot.into_mut(),
            Entry::Vacant(slot) => {
                let (value, bytes) = make()?;
                self.bytes += bytes;
                slot.insert(Cached {
                    value,
                    bytes,
                    used: 0,
                })
            }
        };
        entry.used = self.frame;
        Ok(&entry.value)
    }

    fn remove_where(&mut self, mut doomed: impl FnMut(&K) -> bool, mut release: impl FnMut(V)) {
        for (_, evicted) in self.entries.extract_if(|key, _| doomed(key)) {
            self.bytes -= evicted.bytes;
            release(evicted.value);
        }
    }

    fn evict_to_budget(&mut self, budget: usize, mut release: impl FnMut(V)) {
        if self.bytes <= budget {
            return;
        }
        self.stale.clear();
        self.stale.extend(
            self.entries
                .iter()
                .filter(|(_, c)| c.used < self.frame)
                .map(|(k, c)| (c.used, *k)),
        );
        self.stale.sort_unstable_by_key(|&(used, _)| used);
        for &(_, key) in &self.stale {
            if self.bytes <= budget {
                break;
            }
            let evicted = self
                .entries
                .remove(&key)
                .expect("stale keys come from entries");
            self.bytes -= evicted.bytes;
            release(evicted.value);
        }
    }
}

fn food_shade(c: Rgba) -> Rgba {
    let level = |v: u8| ((u16::from(v) + 8) / 17 * 17) as u8;
    [level(c[0]), level(c[1]), level(c[2]), c[3]]
}

fn css_color(c: Rgba) -> String {
    format!(
        "rgba({},{},{},{:.3})",
        c[0],
        c[1],
        c[2],
        f32::from(c[3]) / 255.0
    )
}

fn glyph_rect(uv: [u16; 4]) -> [f64; 4] {
    let px = |v: u16| f64::from(v) / f64::from(u16::MAX) * f64::from(ATLAS_SIDE);
    let [u0, v0, u1, v1] = uv;
    [px(u0), px(v0), px(u1) - px(u0), px(v1) - px(v0)]
}

fn erfc(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_7 + t * (1.421_413_8 + t * (-1.453_152 + t * 1.061_405_4))));
    poly * (-x * x).exp()
}

pub(crate) struct Canvas2dBackend {
    ctx: CanvasRenderingContext2d,
    food_buckets: Vec<(Rgba, Vec<u32>)>,
    baked: ByteLru<BakedKey, OffscreenCanvas>,
    sources: HashMap<u32, Vec<ImageBitmap>>,
    ladder: Vec<u32>,
    jelly: Vec<f32>,
    skin_generation: Option<u32>,
    fill: Option<Rgba>,
    alpha: f64,
    tinted: GlyphSheet,
    glyph_generation: Option<u32>,
    px: f64,
    pages: Vec<(OffscreenCanvas, OffscreenCanvasRenderingContext2d)>,
    shadow: OffscreenCanvas,
    text_font: String,
    text_family: String,
    text_epoch: u32,
}

struct Bounds {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

fn outline_reach(outline: Outline) -> f32 {
    match outline {
        Outline::Round => 0.0,
        Outline::Jelly(_) => JELLY_REACH,
    }
}

impl Bounds {
    fn overlaps_box(&self, x: f32, y: f32, hw: f32, hh: f32) -> bool {
        x + hw >= self.x0 && x - hw <= self.x1 && y + hh >= self.y0 && y - hh <= self.y1
    }

    fn overlaps_circle(&self, at: Circle, reach: f32) -> bool {
        self.overlaps_box(at.x, at.y, at.r * reach, at.r * reach)
    }

    fn overlaps_prim(&self, p: &Prim) -> bool {
        match *p {
            Prim::Disc { at, outline, .. }
            | Prim::Ring { at, outline, .. }
            | Prim::GradientRing { at, outline, .. } => {
                self.overlaps_circle(at, 1.0 + outline_reach(outline))
            }
            Prim::Card {
                at, outline, glow, ..
            } => self.overlaps_circle(
                at,
                1.0 + GLOW_REACH * glow.map_or(0.0, GlowSigma::sigma) + outline_reach(outline),
            ),
            Prim::Shadow { at, outline, .. } => {
                self.overlaps_circle(at, SHADOW_REACH + outline_reach(outline))
            }
            Prim::Halo { at, .. } => self.overlaps_circle(at, 1.0),
            Prim::Star { at, .. } => self.overlaps_circle(at, STAR_EXTENT),
            Prim::Sdf { .. } => unreachable!("the 2d backend draws vector text instead"),
            Prim::VectorText { x, y, font, .. } => self.overlaps_box(x, y, font * 1.5, font),
            Prim::Glyph { rect, .. }
            | Prim::Rect { rect, .. }
            | Prim::Image { rect, .. }
            | Prim::Frame { rect, .. } => {
                self.overlaps_box(rect.x, rect.y, rect.half_w, rect.half_h)
            }
            Prim::Arrow { x, y, half, .. } => self.overlaps_box(x, y, half, half),
            Prim::Segment { .. } => true,
        }
    }
}

fn shadow_texture() -> Result<OffscreenCanvas, JsValue> {
    let canvas = OffscreenCanvas::new(SHADOW_SIDE, SHADOW_SIDE)?;
    let ctx: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("2d context unavailable")?
        .dyn_into()?;
    let half = f64::from(SHADOW_SIDE) / 2.0;
    let edge = 1.0 / SHADOW_REACH;
    let gradient = ctx.create_radial_gradient(half, half, 0.0, half, half, half)?;
    gradient.add_color_stop((1.0 - SHADOW_FEATHER) * edge, "rgba(0, 0, 0, 0)")?;
    gradient.add_color_stop(edge, "rgba(0, 0, 0, 1)")?;
    gradient.add_color_stop(1.0, "rgba(0, 0, 0, 0)")?;
    ctx.set_fill_style_canvas_gradient(&gradient);
    ctx.fill_rect(0.0, 0.0, half * 2.0, half * 2.0);
    Ok(canvas)
}

fn release_baked(canvas: OffscreenCanvas) {
    canvas.set_width(0);
    canvas.set_height(0);
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bake_mark(color: Rgba, side: u32) -> Result<(OffscreenCanvas, usize), JsValue> {
    let canvas = OffscreenCanvas::new(side, side)?;
    let ctx: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("2d context unavailable")?
        .dyn_into()?;
    let half = side as f32 / 2.0;
    let e = 1.0 / half;
    let bytes = side as usize * side as usize * 4;
    let mut pixels = vec![0u8; bytes];
    for (i, px) in pixels.chunks_exact_mut(4).enumerate() {
        let x = (i as u32 % side) as f32 + 0.5 - half;
        let y = (i as u32 / side) as f32 + 0.5 - half;
        let d = x.hypot(y) / half;
        if d > 1.0 {
            continue;
        }
        let inward = 1.0 - d;
        let glow = 0.5 * (-(inward * inward) / OVERLAP_GLOW_SPREAD).exp();
        let rim = smoothstep(OVERLAP_RIM - e, OVERLAP_RIM + e, d);
        let cover = glow.max(rim) * (1.0 - smoothstep(1.0 - e, 1.0, d));
        px.copy_from_slice(&[
            color[0],
            color[1],
            color[2],
            (f32::from(color[3]) * cover).round() as u8,
        ]);
    }
    let image = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&pixels), side, side)?;
    ctx.put_image_data(&image, 0, 0)?;
    Ok((canvas, bytes))
}

fn bake_glow(
    rgb: [u8; 3],
    sigma: GlowSigma,
    side: u32,
) -> Result<(OffscreenCanvas, usize), JsValue> {
    let canvas = OffscreenCanvas::new(side, side)?;
    let ctx: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("2d context unavailable")?
        .dyn_into()?;
    let full = f64::from(side);
    let c = full / 2.0;
    let r = c / f64::from(1.0 + GLOW_REACH * sigma.sigma());
    let [red, green, blue] = rgb;
    ctx.begin_path();
    ctx.rect(0.0, 0.0, full, full);
    ctx.arc_with_anticlockwise(c, c, (r - GLOW_HOLE_INSET_TEXELS).max(0.0), 0.0, TAU, true)?;
    ctx.clip();
    let caster_offset = full * 2.0;
    ctx.begin_path();
    ctx.arc(c - caster_offset, c, r, 0.0, TAU)?;
    ctx.set_fill_style_str(&css_color([red, green, blue, 255]));
    ctx.set_shadow_color(&css_color([red, green, blue, 255]));
    ctx.set_shadow_offset_x(caster_offset);
    ctx.set_shadow_blur(2.0 * f64::from(sigma.sigma()) * r);
    ctx.fill();
    Ok((canvas, side as usize * side as usize * 4))
}

fn bake_card(
    key: &CardKey,
    sources: &HashMap<u32, Vec<ImageBitmap>>,
) -> Result<(OffscreenCanvas, usize), JsValue> {
    let side = key.canvas_side();
    let canvas = OffscreenCanvas::new(side, side)?;
    let ctx: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("2d context unavailable")?
        .dyn_into()?;
    let full = f64::from(side);
    let c = full / 2.0;
    let r = f64::from(key.side) / 2.0;
    let cover = |ctx: &OffscreenCanvasRenderingContext2d| match key.cut {
        Cut::Disc => ctx.fill(),
        Cut::Bleed => ctx.fill_rect(0.0, 0.0, full, full),
    };
    let [r0, g0, b0] = key.body;
    ctx.set_fill_style_str(&css_color([r0, g0, b0, 255]));
    ctx.begin_path();
    ctx.arc(c, c, r, 0.0, TAU)?;
    cover(&ctx);
    if let Some((layer, alpha)) = key.skin {
        let mip = sources[&layer]
            .iter()
            .find(|m| m.width() == key.side)
            .expect("skin mip");
        ctx.save();
        ctx.clip();
        ctx.set_global_alpha(f64::from(alpha) / 255.0);
        ctx.draw_image_with_image_bitmap(mip, c - r, c - r)?;
        ctx.restore();
    }
    match key.decor {
        Decor::Enemy(team) => {
            ctx.set_global_alpha(f64::from(ENEMY_DIM));
            ctx.set_fill_style_str("#000");
            cover(&ctx);
            let d = (std::f64::consts::FRAC_1_SQRT_2 - f64::from(ENEMY_RING_INSET)) * r;
            ctx.set_global_alpha(f64::from(team[3]) / 255.0);
            ctx.set_stroke_style_str(&css_color([team[0], team[1], team[2], 255]));
            ctx.set_line_width(r * f64::from(ENEMY_LINE));
            ctx.begin_path();
            ctx.arc(c, c, r * (1.0 - f64::from(ENEMY_RING_INSET)), 0.0, TAU)?;
            ctx.stroke();
            ctx.begin_path();
            ctx.move_to(c + d, c - d);
            ctx.line_to(c - d, c + d);
            ctx.stroke();
        }
        Decor::Tint(tint) => {
            ctx.set_fill_style_str(&css_color(tint));
            cover(&ctx);
        }
        Decor::None => {}
    }
    Ok((canvas, side as usize * side as usize * 4))
}

fn jelly_stride(radius_px: f32) -> usize {
    let wanted = std::f32::consts::PI * (2.0 * radius_px.max(0.0) / JELLY_CHORD_ERROR_PX).sqrt();
    let mut samples = JELLY_MIN_SAMPLES;
    while samples < JELLY_SAMPLES && (samples as f32) < wanted {
        samples *= 2;
    }
    JELLY_SAMPLES / samples
}

fn trace_outline(
    ctx: &CanvasRenderingContext2d,
    jelly: &[f32],
    px: f64,
    at: Circle,
    outline: Outline,
    radius: f32,
    reverse: bool,
) -> Result<(), JsValue> {
    let (x, y, r) = (f64::from(at.x), f64::from(at.y), at.r);
    let Outline::Jelly(JellyRow(row)) = outline else {
        return ctx.arc_with_anticlockwise(x, y, f64::from(radius), 0.0, TAU, reverse);
    };
    let start = usize::from(row) * JELLY_ROW;
    let q = &jelly[start..start + JELLY_SAMPLES];
    let stride = jelly_stride(radius * px as f32);
    let count = JELLY_SAMPLES / stride;
    let mut points = [0.0_f32; JELLY_SAMPLES * 2];
    for (step, point) in points.chunks_exact_mut(2).take(count).enumerate() {
        let k = if reverse {
            JELLY_SAMPLES - stride * (step + 1)
        } else {
            stride * step
        };
        let angle = k as f64 / JELLY_SAMPLES as f64 * TAU;
        let rho = f64::from(jelly::radius(radius, r, q[k] * r));
        point[0] = (x + angle.cos() * rho) as f32;
        point[1] = (y + angle.sin() * rho) as f32;
    }
    trace_polygon(ctx, &points[..count * 2]);
    Ok(())
}

impl Canvas2dBackend {
    pub(crate) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"alpha".into(), &false.into())?;
        let ctx: CanvasRenderingContext2d = canvas
            .get_context_with_context_options("2d", &options)?
            .ok_or("2d context unavailable")?
            .dyn_into()?;
        Ok(Self {
            ctx,
            food_buckets: Vec::new(),
            baked: ByteLru::new(),
            sources: HashMap::new(),
            ladder: Vec::new(),
            jelly: Vec::new(),
            skin_generation: None,
            fill: None,
            alpha: 1.0,
            tinted: GlyphSheet::default(),
            glyph_generation: None,
            px: 1.0,
            pages: Vec::new(),
            shadow: shadow_texture()?,
            text_font: String::new(),
            text_family: String::new(),
            text_epoch: 0,
        })
    }

    fn set_fill(&mut self, c: Rgba) {
        if self.fill != Some(c) {
            self.ctx.set_fill_style_str(&css_color(c));
            self.fill = Some(c);
        }
    }

    fn set_alpha(&mut self, alpha: f64) {
        if self.alpha != alpha {
            self.ctx.set_global_alpha(alpha);
            self.alpha = alpha;
        }
    }

    fn bake_tints(&mut self, prims: &[Prim], bounds: &Bounds) -> Result<(), JsValue> {
        for p in prims.iter().filter(|p| bounds.overlaps_prim(p)) {
            if let Prim::Glyph {
                uv,
                page,
                tint: [r, g, b, _],
                ..
            } = *p
            {
                if [r, g, b] != [255; 3] {
                    let key = TintKey {
                        uv,
                        page,
                        rgb: [r, g, b],
                    };
                    self.tinted.ensure(key, &self.pages[page as usize].0)?;
                }
            }
        }
        Ok(())
    }

    fn upload_pending_assets(
        &mut self,
        atlas: &mut TextAtlas,
        skins: &mut SkinCache,
        food: &[Prim],
        food_changed: bool,
    ) -> Result<(), JsValue> {
        while (self.pages.len() as u32) < atlas.pages() {
            let page = OffscreenCanvas::new(ATLAS_SIDE, ATLAS_SIDE)?;
            let ctx: OffscreenCanvasRenderingContext2d = page
                .get_context("2d")?
                .ok_or("2d context unavailable")?
                .dyn_into()?;
            self.pages.push((page, ctx));
        }
        for u in atlas.take_uploads() {
            let (_, page) = &self.pages[u.page as usize];
            page.clear_rect(
                f64::from(u.x),
                f64::from(u.y),
                f64::from(u.bitmap.width()),
                f64::from(u.bitmap.height()),
            );
            page.draw_image_with_image_bitmap(&u.bitmap, f64::from(u.x), f64::from(u.y))?;
            u.bitmap.close();
        }
        if self.glyph_generation != Some(atlas.generation()) {
            self.tinted.clear();
            self.glyph_generation = Some(atlas.generation());
        }
        if self.skin_generation != Some(skins.generation()) {
            self.baked.remove_where(|_| true, release_baked);
            self.sources
                .drain()
                .flat_map(|(_, mips)| mips)
                .for_each(|m| m.close());
            self.ladder = mip_sides(skins.side());
            self.skin_generation = Some(skins.generation());
        }
        for upload in skins.take_uploads() {
            let layer = upload.layer;
            self.baked.remove_where(
                |key| matches!(key, BakedKey::Card(c) if c.skin.is_some_and(|(l, _)| l == layer)),
                release_baked,
            );
            if let Some(old) = self.sources.insert(layer, upload.mips) {
                old.iter().for_each(ImageBitmap::close);
            }
        }
        if food_changed {
            let mut index: HashMap<Rgba, usize> = HashMap::new();
            self.food_buckets.clear();
            for (i, p) in food.iter().enumerate() {
                let Prim::Disc { fill, .. } = *p else {
                    unreachable!("food is built from discs only")
                };
                let shade = food_shade(fill);
                let slot = *index.entry(shade).or_insert_with(|| {
                    self.food_buckets.push((shade, Vec::new()));
                    self.food_buckets.len() - 1
                });
                self.food_buckets[slot].1.push(i as u32);
            }
        }
        Ok(())
    }

    fn draw_disc(
        &mut self,
        at: Circle,
        outline: Outline,
        fill: Rgba,
        edge: Option<Edge>,
    ) -> Result<(), JsValue> {
        if let Outline::Jelly(_) = outline {
            self.set_fill(fill);
            self.ctx.begin_path();
            trace_outline(&self.ctx, &self.jelly, self.px, at, outline, at.r, false)?;
            self.ctx.fill();
            if let Some(e) = edge {
                self.ctx.set_fill_style_str(&css_color(e.color));
                self.fill = None;
                self.ctx.begin_path();
                trace_outline(&self.ctx, &self.jelly, self.px, at, outline, at.r, false)?;
                trace_outline(
                    &self.ctx,
                    &self.jelly,
                    self.px,
                    at,
                    outline,
                    at.r * (1.0 - e.width),
                    true,
                )?;
                self.ctx.fill();
            }
            return Ok(());
        }
        let (x, y, r) = (f64::from(at.x), f64::from(at.y), f64::from(at.r));
        self.set_fill(fill);
        let ctx = &self.ctx;
        ctx.begin_path();
        ctx.arc(x, y, r, 0.0, TAU)?;
        ctx.fill();
        if let Some(e) = edge {
            let w = r * f64::from(e.width);
            ctx.set_stroke_style_str(&css_color(e.color));
            ctx.set_line_width(w);
            ctx.begin_path();
            ctx.arc(x, y, r - w / 2.0, 0.0, TAU)?;
            ctx.stroke();
        }
        Ok(())
    }

    fn draw_card(
        &mut self,
        at: Circle,
        outline: Outline,
        look: &CardLook,
        glow: Option<GlowSigma>,
    ) -> Result<(), JsValue> {
        self.fill = None;
        if let Some(sigma) = glow {
            self.draw_glow(at, look, sigma)?;
        }
        if let Some(fill) = look.flat() {
            self.set_alpha(1.0);
            self.ctx.begin_path();
            trace_outline(&self.ctx, &self.jelly, self.px, at, outline, at.r, false)?;
            self.ctx.set_fill_style_str(&css_color(fill));
            self.ctx.fill();
            return Ok(());
        }
        let (side, fits) = pick_mip_side(&self.ladder, 2.0 * at.r * self.px as f32);
        let key = CardKey {
            skin: look
                .skin
                .map(|s| (s.layer, (s.alpha * 255.0).round() as u8)),
            body: look.body,
            decor: look.decor,
            side,
            cut: match outline {
                Outline::Round if fits => Cut::Disc,
                Outline::Round | Outline::Jelly(_) => Cut::Bleed,
            },
        };
        let sources = &self.sources;
        let card = self
            .baked
            .get_or_try_insert(BakedKey::Card(key), || bake_card(&key, sources))?;
        let r = f64::from(at.r);
        let half = r * f64::from(key.canvas_side()) / f64::from(side);
        let (ctx, alpha) = (&self.ctx, f64::from(look.alpha));
        if self.alpha != alpha {
            ctx.set_global_alpha(alpha);
            self.alpha = alpha;
        }
        if key.cut == Cut::Bleed {
            ctx.save();
            ctx.begin_path();
            trace_outline(ctx, &self.jelly, self.px, at, outline, at.r, false)?;
            ctx.clip();
        }
        ctx.draw_image_with_offscreen_canvas_and_dw_and_dh(
            card,
            f64::from(at.x) - half,
            f64::from(at.y) - half,
            half * 2.0,
            half * 2.0,
        )?;
        if key.cut == Cut::Bleed {
            ctx.restore();
        }
        Ok(())
    }

    fn draw_glow(&mut self, at: Circle, look: &CardLook, sigma: GlowSigma) -> Result<(), JsValue> {
        let reach = at.r * (1.0 + GLOW_REACH * sigma.sigma());
        let (side, _) = pick_mip_side(&self.ladder, 2.0 * reach * self.px as f32);
        let glow = self.baked.get_or_try_insert(
            BakedKey::Glow {
                rgb: look.body,
                sigma,
                side,
            },
            || bake_glow(look.body, sigma, side),
        )?;
        let half = f64::from(reach);
        let alpha = f64::from(look.alpha);
        if self.alpha != alpha {
            self.ctx.set_global_alpha(alpha);
            self.alpha = alpha;
        }
        self.ctx.draw_image_with_offscreen_canvas_and_dw_and_dh(
            glow,
            f64::from(at.x) - half,
            f64::from(at.y) - half,
            half * 2.0,
            half * 2.0,
        )
    }

    fn draw_frame(
        &mut self,
        rect: Rect,
        band: f32,
        color: Rgba,
        edge: FrameEdge,
    ) -> Result<(), JsValue> {
        let (l, t, r, b) = (
            rect.x - rect.half_w,
            rect.y - rect.half_h,
            rect.x + rect.half_w,
            rect.y + rect.half_h,
        );
        if let FrameEdge::Rainbow { to, .. } = edge {
            let g = self.ctx.create_linear_gradient(
                f64::from(l - band),
                f64::from(b + band),
                f64::from(r + band),
                f64::from(t - band),
            );
            g.add_color_stop(0.0, &css_color(color))?;
            g.add_color_stop(1.0, &css_color(to))?;
            self.ctx.set_fill_style_canvas_gradient(&g);
            self.fill = None;
        } else {
            self.set_fill(color);
        }
        for (x, y, w, h) in [
            (l - band, t - band, r - l + band * 2.0, band),
            (l - band, b, r - l + band * 2.0, band),
            (l - band, t, band, b - t),
            (r, t, band, b - t),
        ] {
            self.ctx
                .fill_rect(f64::from(x), f64::from(y), f64::from(w), f64::from(h));
        }
        if edge != FrameEdge::Glow {
            return Ok(());
        }
        let sigma = FRAME_GLOW_SIGMA * band;
        let reach = 3.0 * sigma;
        for (x0, y0, x1, y1, area) in [
            (l, t, l + reach, t, (l, t, reach, b - t)),
            (r, t, r - reach, t, (r - reach, t, reach, b - t)),
            (l, t, l, t + reach, (l, t, r - l, reach)),
            (l, b, l, b - reach, (l, b - reach, r - l, reach)),
        ] {
            let g = self.ctx.create_linear_gradient(
                f64::from(x0),
                f64::from(y0),
                f64::from(x1),
                f64::from(y1),
            );
            for k in 0..=FRAME_GLOW_STOPS {
                let f = k as f32 / FRAME_GLOW_STOPS as f32;
                let a = 0.5
                    * erfc(f * reach / (sigma * std::f32::consts::SQRT_2))
                    * f32::from(color[3])
                    / 255.0;
                g.add_color_stop(
                    f,
                    &css_color([color[0], color[1], color[2], (a * 255.0).round() as u8]),
                )?;
            }
            self.ctx.set_fill_style_canvas_gradient(&g);
            self.ctx.fill_rect(
                f64::from(area.0),
                f64::from(area.1),
                f64::from(area.2),
                f64::from(area.3),
            );
        }
        self.fill = None;
        Ok(())
    }

    fn draw_gradient_ring(
        &mut self,
        at: Circle,
        outline: Outline,
        width: f32,
        slope: f32,
        from: Rgba,
        to: Rgba,
    ) -> Result<(), JsValue> {
        let (x, y, r) = (f64::from(at.x), f64::from(at.y), f64::from(at.r));
        let reach = r / (4.0 * f64::from(slope));
        let g = self
            .ctx
            .create_linear_gradient(x - reach, y + reach, x + reach, y - reach);
        g.add_color_stop(0.0, &css_color(from))?;
        g.add_color_stop(1.0, &css_color(to))?;
        self.fill = None;
        let ctx = &self.ctx;
        ctx.begin_path();
        trace_outline(ctx, &self.jelly, self.px, at, outline, at.r, false)?;
        if width < 1.0 {
            trace_outline(
                ctx,
                &self.jelly,
                self.px,
                at,
                outline,
                at.r * (1.0 - width),
                true,
            )?;
        }
        ctx.set_fill_style_canvas_gradient(&g);
        ctx.fill();
        Ok(())
    }

    fn draw_overlap(&mut self, m: &OverlapMark) -> Result<(), JsValue> {
        let (x, y, r) = (f64::from(m.x), f64::from(m.y), f64::from(m.r));
        let ctx = &self.ctx;
        ctx.save();
        let mut clip = m.clip_circles().peekable();
        if clip.peek().is_some() {
            ctx.begin_path();
            for [dx, dy, cr] in clip {
                let (cx, cy, cr) = (x + f64::from(dx), y + f64::from(dy), f64::from(cr));
                ctx.move_to(cx + cr, cy);
                ctx.arc(cx, cy, cr, 0.0, TAU)?;
            }
            ctx.clip();
        }
        let (side, fits) = pick_mip_side(&self.ladder, 2.0 * m.r * self.px as f32);
        if fits {
            let mark = self.baked.get_or_try_insert(
                BakedKey::Mark {
                    color: m.color,
                    side,
                },
                || bake_mark(m.color, side),
            )?;
            ctx.draw_image_with_offscreen_canvas_and_dw_and_dh(
                mark,
                x - r,
                y - r,
                r * 2.0,
                r * 2.0,
            )?;
            ctx.restore();
            return Ok(());
        }
        let e = (1.0 / (r * self.px)).min(0.007);
        let spread = f64::from(OVERLAP_GLOW_SPREAD);
        let rim = f64::from(OVERLAP_RIM);
        let glow = |d: f64| 0.5 * (-(1.0 - d).powi(2) / spread).exp();
        let g = ctx.create_radial_gradient(x, y, 0.0, x, y, r)?;
        let rgb = [m.color[0], m.color[1], m.color[2]];
        let stop = |t: f64, a: f64| {
            let alpha = (a * f64::from(m.color[3])).round() as u8;
            g.add_color_stop(t as f32, &css_color([rgb[0], rgb[1], rgb[2], alpha]))
        };
        let rim_start = rim - e;
        for k in 0..OVERLAP_STOPS {
            let t = rim_start * k as f64 / OVERLAP_STOPS as f64;
            stop(t, glow(t))?;
        }
        stop(rim_start, glow(rim_start))?;
        stop(rim + e, 1.0)?;
        stop(1.0 - e, 1.0)?;
        stop(1.0, 0.0)?;
        ctx.set_fill_style_canvas_gradient(&g);
        ctx.begin_path();
        ctx.arc(x, y, r, 0.0, TAU)?;
        ctx.fill();
        ctx.restore();
        self.fill = None;
        Ok(())
    }

    fn draw_shadow(&mut self, at: Circle, color: Rgba) -> Result<(), JsValue> {
        let r = f64::from(at.r * SHADOW_REACH);
        self.set_alpha(f64::from(color[3]) / 255.0);
        self.ctx.draw_image_with_offscreen_canvas_and_dw_and_dh(
            &self.shadow,
            f64::from(at.x) - r,
            f64::from(at.y) - r,
            r * 2.0,
            r * 2.0,
        )?;
        Ok(())
    }

    fn draw_halo(&mut self, at: Circle, color: Rgba) -> Result<(), JsValue> {
        let (x, y, r) = (f64::from(at.x), f64::from(at.y), f64::from(at.r));
        let gradient = self
            .ctx
            .create_radial_gradient(x, y, r * f64::from(HALO_INNER), x, y, r)?;
        gradient.add_color_stop(0.0, "transparent")?;
        gradient.add_color_stop(1.0, &css_color(color))?;
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.fill = None;
        self.ctx.begin_path();
        self.ctx.arc(x, y, r, 0.0, TAU)?;
        self.ctx.fill();
        Ok(())
    }

    fn draw_star(&mut self, at: Circle, fill: Rgba, stroke: Rgba) -> Result<(), JsValue> {
        let r = f64::from(at.r);
        let (x, y) = (f64::from(at.x), f64::from(at.y));
        let step = TAU / STAR_POINTS as f64;
        let ctx = &self.ctx;
        ctx.begin_path();
        for n in 0..=STAR_POINTS {
            let a = n as f64 * step;
            let outer = f64::from(STAR_OUTER) * r;
            let inner = f64::from(STAR_INNER) * r;
            ctx.line_to(x + a.cos() * outer, y - a.sin() * outer);
            ctx.line_to(
                x + (a + step / 2.0).cos() * inner,
                y - (a + step / 2.0).sin() * inner,
            );
        }
        ctx.close_path();
        self.set_fill(fill);
        let ctx = &self.ctx;
        ctx.fill();
        ctx.set_stroke_style_str(&css_color(stroke));
        ctx.set_line_width(f64::from(STAR_LINE) * r);
        ctx.set_line_join("round");
        ctx.stroke();
        Ok(())
    }

    fn draw_glyph(
        &mut self,
        rect: Rect,
        uv: [u16; 4],
        page: u32,
        tint: Rgba,
    ) -> Result<(), JsValue> {
        self.set_alpha(f64::from(tint[3]) / 255.0);
        let [sx, sy, sw, sh] = glyph_rect(uv);
        let (source, sx, sy) = match tint {
            [255, 255, 255, _] => (&self.pages[page as usize].0, sx, sy),
            [r, g, b, _] => {
                let (sheet, slot) = self.tinted.slot(&TintKey {
                    uv,
                    page,
                    rgb: [r, g, b],
                });
                (sheet, slot.x, slot.y)
            }
        };
        self.ctx
            .draw_image_with_offscreen_canvas_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                source,
                sx,
                sy,
                sw,
                sh,
                f64::from(rect.x - rect.half_w),
                f64::from(rect.y - rect.half_h),
                f64::from(rect.half_w * 2.0),
                f64::from(rect.half_h * 2.0),
            )?;
        Ok(())
    }

    fn draw_vector_text(
        &mut self,
        (x, y): (f32, f32),
        font: f32,
        text: &str,
        tint: Rgba,
    ) -> Result<(), JsValue> {
        self.set_alpha(f64::from(tint[3]) / 255.0);
        self.set_fill([tint[0], tint[1], tint[2], 255]);
        let k = f64::from(font / SDF_PX);
        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(f64::from(x), f64::from(y))?;
        ctx.scale(k, k)?;
        ctx.set_font(&self.text_font);
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let drawn = ctx.fill_text(text, 0.0, 0.0);
        ctx.restore();
        drawn
    }

    fn draw_image(
        &mut self,
        rect: Rect,
        uv: [u16; 4],
        opacity: u8,
        backdrop: &Backdrop,
    ) -> Result<(), JsValue> {
        let image = backdrop
            .mips()
            .and_then(<[_]>::first)
            .expect("backdrop image");
        let (w, h) = (f64::from(image.width()), f64::from(image.height()));
        let u = |v: u16| f64::from(v) / f64::from(u16::MAX);
        self.set_alpha(f64::from(opacity) / 255.0);
        self.ctx
            .draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                image,
                u(uv[0]) * w,
                u(uv[1]) * h,
                (u(uv[2]) - u(uv[0])) * w,
                (u(uv[3]) - u(uv[1])) * h,
                f64::from(rect.x - rect.half_w),
                f64::from(rect.y - rect.half_h),
                f64::from(rect.half_w * 2.0),
                f64::from(rect.half_h * 2.0),
            )?;
        Ok(())
    }

    fn draw_list(
        &mut self,
        prims: &[Prim],
        bounds: &Bounds,
        backdrop: &Backdrop,
    ) -> Result<(), JsValue> {
        for p in prims.iter().filter(|p| bounds.overlaps_prim(p)) {
            if !matches!(
                p,
                Prim::Glyph { .. }
                    | Prim::VectorText { .. }
                    | Prim::Card { .. }
                    | Prim::Image { .. }
                    | Prim::Shadow { .. }
            ) {
                self.set_alpha(1.0);
            }
            match *p {
                Prim::Disc {
                    at,
                    outline,
                    fill,
                    edge,
                } => self.draw_disc(at, outline, fill, edge)?,
                Prim::Ring { at, outline, edge } => {
                    self.draw_disc(at, outline, [0; 4], Some(edge))?
                }
                Prim::Card {
                    at,
                    outline,
                    look,
                    glow,
                } => self.draw_card(at, outline, &look, glow)?,
                Prim::Glyph {
                    rect,
                    uv,
                    page,
                    tint,
                } => self.draw_glyph(rect, uv, page, tint)?,
                Prim::Sdf { .. } => unreachable!("the 2d backend draws vector text instead"),
                Prim::VectorText {
                    x,
                    y,
                    font,
                    text,
                    tint,
                } => self.draw_vector_text((x, y), font, text, tint)?,
                Prim::Star { at, fill, stroke } => self.draw_star(at, fill, stroke)?,
                Prim::Halo { at, color } => self.draw_halo(at, color)?,
                Prim::GradientRing {
                    at,
                    outline,
                    width,
                    slope,
                    from,
                    to,
                } => self.draw_gradient_ring(at, outline, width, slope, from, to)?,
                Prim::Shadow { at, color, .. } => self.draw_shadow(at, color)?,
                Prim::Frame {
                    rect,
                    band,
                    color,
                    edge,
                } => self.draw_frame(rect, band, color, edge)?,
                Prim::Image { rect, uv, opacity } => {
                    self.draw_image(rect, uv, opacity, backdrop)?
                }
                Prim::Arrow { x, y, half, color } => {
                    let (x, y, h) = (f64::from(x), f64::from(y), f64::from(half));
                    self.set_fill(color);
                    self.ctx.begin_path();
                    self.ctx.move_to(x, y + f64::from(ARROW_TIP) * h);
                    self.ctx.line_to(
                        x - f64::from(ARROW_HALF_WIDTH) * h,
                        y + f64::from(ARROW_BASE) * h,
                    );
                    self.ctx.line_to(
                        x + f64::from(ARROW_HALF_WIDTH) * h,
                        y + f64::from(ARROW_BASE) * h,
                    );
                    self.ctx.close_path();
                    self.ctx.fill();
                }
                Prim::Segment {
                    from,
                    to,
                    width,
                    color,
                } => {
                    self.ctx.set_stroke_style_str(&css_color(color));
                    self.ctx.set_line_width(f64::from(width));
                    self.ctx.begin_path();
                    self.ctx.move_to(f64::from(from.0), f64::from(from.1));
                    self.ctx.line_to(f64::from(to.0), f64::from(to.1));
                    self.ctx.stroke();
                }
                Prim::Rect { rect, color } => {
                    self.set_fill(color);
                    self.ctx.fill_rect(
                        f64::from(rect.x - rect.half_w),
                        f64::from(rect.y - rect.half_h),
                        f64::from(rect.half_w * 2.0),
                        f64::from(rect.half_h * 2.0),
                    );
                }
            }
        }
        self.set_alpha(1.0);
        Ok(())
    }

    fn draw_food(&mut self, food: &[Prim], bounds: &Bounds) -> Result<(), JsValue> {
        let buckets = std::mem::take(&mut self.food_buckets);
        for (color, indices) in &buckets {
            self.set_fill(*color);
            for p in indices
                .iter()
                .map(|&i| &food[i as usize])
                .filter(|p| bounds.overlaps_prim(p))
            {
                let Prim::Disc { at, .. } = *p else {
                    unreachable!("food is built from discs only")
                };
                let (x, y, r) = (f64::from(at.x), f64::from(at.y), f64::from(at.r));
                if r * self.px <= FOOD_SQUARE_MAX_RADIUS_PX {
                    let half = r * std::f64::consts::PI.sqrt() / 2.0;
                    self.ctx
                        .fill_rect(x - half, y - half, half * 2.0, half * 2.0);
                } else {
                    self.ctx.begin_path();
                    self.ctx.arc(x, y, r, 0.0, TAU)?;
                    self.ctx.fill();
                }
            }
        }
        self.food_buckets = buckets;
        Ok(())
    }

    pub(crate) fn draw(
        &mut self,
        view: &FrameView,
        layers: &Layers,
        food_changed: bool,
        atlas: &mut TextAtlas,
        skins: &mut SkinCache,
        backdrop: &Backdrop,
    ) -> Result<(), JsValue> {
        let &Layers {
            under,
            border,
            shadows,
            food,
            dots,
            cells,
            over,
            minimap,
            minimap_clip,
            clip,
            ..
        } = layers;
        self.jelly.clear();
        self.jelly.extend_from_slice(layers.jelly);
        self.upload_pending_assets(atlas, skins, food, food_changed)?;
        if self.text_epoch != atlas.font_epoch() || self.text_family != atlas.coord_family() {
            self.text_epoch = atlas.font_epoch();
            atlas.coord_family().clone_into(&mut self.text_family);
            self.text_font = format!("{SDF_PX}px {}", self.text_family);
        }
        let (hw, hh) = (
            view.width as f32 / 2.0 / view.scale,
            view.height as f32 / 2.0 / view.scale,
        );
        let bounds = Bounds {
            x0: view.cam_x - hw,
            y0: view.cam_y - hh,
            x1: view.cam_x + hw,
            y1: view.cam_y + hh,
        };
        let screen = Bounds {
            x0: 0.0,
            y0: 0.0,
            x1: view.width as f32,
            y1: view.height as f32,
        };
        self.tinted.begin_frame();
        for list in [under, border, shadows, dots, cells, over] {
            self.bake_tints(list, &bounds)?;
        }
        for list in [minimap, minimap_clip] {
            self.bake_tints(list, &screen)?;
        }
        let ctx = &self.ctx;
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)?;
        if view.background[3] < u8::MAX {
            ctx.clear_rect(0.0, 0.0, f64::from(view.width), f64::from(view.height));
        }
        ctx.set_fill_style_str(&css_color(view.background));
        ctx.fill_rect(0.0, 0.0, f64::from(view.width), f64::from(view.height));
        self.fill = Some(view.background);
        let scale = f64::from(view.scale);
        self.px = scale;
        self.baked.next_frame();
        ctx.set_transform(
            scale,
            0.0,
            0.0,
            scale,
            f64::from(view.width) / 2.0 - f64::from(view.cam_x) * scale,
            f64::from(view.height) / 2.0 - f64::from(view.cam_y) * scale,
        )?;
        self.draw_list(under, &bounds, backdrop)?;
        self.ctx.set_global_composite_operation("screen")?;
        self.draw_list(border, &bounds, backdrop)?;
        self.ctx.set_global_composite_operation("source-over")?;
        self.draw_list(shadows, &bounds, backdrop)?;
        self.draw_food(food, &bounds)?;
        self.draw_list(dots, &bounds, backdrop)?;
        self.draw_list(cells, &bounds, backdrop)?;
        for mark in layers.overlaps {
            self.draw_overlap(mark)?;
        }
        self.draw_list(over, &bounds, backdrop)?;
        if !minimap.is_empty() || !minimap_clip.is_empty() {
            self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)?;
            self.px = 1.0;
            self.draw_list(minimap, &screen, backdrop)?;
            if let Some(c) = clip.filter(|_| !minimap_clip.is_empty()) {
                self.ctx.save();
                self.ctx.begin_path();
                self.ctx.rect(
                    f64::from(c.x),
                    f64::from(c.y),
                    f64::from(c.w),
                    f64::from(c.h),
                );
                self.ctx.clip();
                self.draw_list(minimap_clip, &screen, backdrop)?;
                self.ctx.restore();
                self.fill = None;
            }
        }
        self.baked.evict_to_budget(BAKED_BUDGET, release_baked);
        Ok(())
    }
}
