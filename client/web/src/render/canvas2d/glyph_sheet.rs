use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};

use super::{css_color, glyph_rect};
use crate::render::text::{ATLAS_SIDE, GUTTER};

const SHEET_SIDE: u32 = 2048;
const SHELF_SLACK: u32 = 4;
const MAX_PAGES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct TintKey {
    pub uv: [u16; 4],
    pub page: u32,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TintSlot {
    pub page: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Shelf {
    y: u32,
    height: u32,
    used: u32,
}

#[derive(Debug, Default)]
struct Shelves {
    shelves: Vec<Shelf>,
    bottom: u32,
}

impl Shelves {
    fn place(&mut self, w: u32, h: u32, side: u32) -> Option<(u32, u32)> {
        let fits =
            |s: &&mut Shelf| h <= s.height && s.height <= h + SHELF_SLACK && s.used + w <= side;
        if let Some(shelf) = self.shelves.iter_mut().find(|s| fits(s)) {
            let x = shelf.used;
            shelf.used += w;
            return Some((x, shelf.y));
        }
        if w > side || self.bottom + h > side {
            return None;
        }
        let y = self.bottom;
        self.bottom += h;
        self.shelves.push(Shelf {
            y,
            height: h,
            used: w,
        });
        Some((0, y))
    }
}

struct Page {
    canvas: OffscreenCanvas,
    ctx: OffscreenCanvasRenderingContext2d,
    shelves: Shelves,
}

impl Page {
    fn new() -> Result<Self, JsValue> {
        let canvas = OffscreenCanvas::new(SHEET_SIDE, SHEET_SIDE)?;
        let ctx = canvas
            .get_context("2d")?
            .ok_or("2d context unavailable")?
            .dyn_into()?;
        Ok(Self {
            canvas,
            ctx,
            shelves: Shelves::default(),
        })
    }

    fn clear(&mut self) {
        let side = f64::from(SHEET_SIDE);
        self.ctx.clear_rect(0.0, 0.0, side, side);
        self.shelves = Shelves::default();
    }

    fn bake(
        &self,
        atlas: &OffscreenCanvas,
        rect: [f64; 4],
        x: f64,
        y: f64,
        rgb: [u8; 3],
    ) -> Result<(), JsValue> {
        let [sx, sy, sw, sh] = rect;
        let ctx = &self.ctx;
        let copy = || {
            ctx.draw_image_with_offscreen_canvas_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                atlas, sx, sy, sw, sh, x, y, sw, sh,
            )
        };
        ctx.save();
        ctx.begin_path();
        ctx.rect(x, y, sw.ceil(), sh.ceil());
        ctx.clip();
        copy()?;
        ctx.set_global_composite_operation("multiply")?;
        ctx.set_fill_style_str(&css_color([rgb[0], rgb[1], rgb[2], 255]));
        ctx.fill_rect(x, y, sw.ceil(), sh.ceil());
        ctx.set_global_composite_operation("destination-in")?;
        copy()?;
        ctx.restore();
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct GlyphSheet {
    pages: Vec<Page>,
    slots: HashMap<TintKey, TintSlot>,
}

impl GlyphSheet {
    pub(super) fn clear(&mut self) {
        self.slots.clear();
        // Dropped pages shrink their canvas now instead of holding it until GC.
        let kept = self.pages.len().min(1);
        for page in self.pages.split_off(kept) {
            super::release_baked(page.canvas);
        }
        self.pages.iter_mut().for_each(Page::clear);
    }

    pub(super) fn begin_frame(&mut self) {
        if self.pages.len() > MAX_PAGES {
            self.clear();
        }
    }

    pub(super) fn ensure(&mut self, key: TintKey, atlas: &OffscreenCanvas) -> Result<(), JsValue> {
        if self.slots.contains_key(&key) {
            return Ok(());
        }
        let rect = glyph_rect(key.uv);
        let (w, h) = (
            rect[2].ceil() as u32 + GUTTER,
            rect[3].ceil() as u32 + GUTTER,
        );
        let spot = self
            .pages
            .iter_mut()
            .enumerate()
            .find_map(|(i, p)| p.shelves.place(w, h, SHEET_SIDE).map(|at| (i, at)));
        let (page, (x, y)) = match spot {
            Some(found) => found,
            None => {
                let mut fresh = Page::new()?;
                let at = fresh
                    .shelves
                    .place(w, h, SHEET_SIDE)
                    .expect("glyph fits an empty page");
                self.pages.push(fresh);
                (self.pages.len() - 1, at)
            }
        };
        let (x, y) = (f64::from(x), f64::from(y));
        self.pages[page].bake(atlas, rect, x, y, key.rgb)?;
        self.slots.insert(key, TintSlot { page, x, y });
        Ok(())
    }

    pub(super) fn slot(&self, key: &TintKey) -> (&OffscreenCanvas, TintSlot) {
        let slot = self.slots[key];
        (&self.pages[slot.page].canvas, slot)
    }
}

const _: () = assert!(
    ATLAS_SIDE <= SHEET_SIDE,
    "every text atlas glyph fits a sheet page"
);
