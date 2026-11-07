use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use ogar_core::world::Sym;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, ImageBitmap, OffscreenCanvas, OffscreenCanvasRenderingContext2d, Worker};

pub(crate) const ATLAS_SIDE: u32 = 2048;
pub(crate) const TIERS: [f32; 4] = [13.0, 26.0, 52.0, 104.0];
pub(crate) const SDF_PX: f32 = 256.0;
pub(crate) const SDF_SPREAD: f32 = 4.0;
const SDF_SUPERSAMPLE: f32 = 4.0;
const SHELF_SLACK: u32 = 4;
pub(crate) const GUTTER: u32 = 2;
pub(crate) const DIGITS: &str = "0123456789.K";
const LABEL_FAMILIES: &str = "Overpass, \"Noto Sans\", Meiryo, sans-serif";
const CIRCULAR_FAMILIES: &str = "Meiryo, Arial";
pub(crate) const LABEL_FAMILY: &str = "Overpass";
const CANVAS_FAMILIES: [&str; 4] = [LABEL_FAMILY, "Noto Sans", "Meiryo", "Arial"];
pub(crate) const PRELOAD_FONTS: [&str; 2] = ["600 16px Overpass", "bold 16px Overpass"];

#[wasm_bindgen(module = "/src/render/text_raster.js")]
extern "C" {
    fn render(req: &JsValue) -> JsValue;
    #[wasm_bindgen(js_name = startWorker, catch)]
    fn start_worker(
        on_result: &js_sys::Function,
        on_failure: &js_sys::Function,
        on_fonts: &js_sys::Function,
        families: &js_sys::Array,
    ) -> Result<Worker, JsValue>;
    #[wasm_bindgen(js_name = addFaces)]
    fn add_faces(worker: &Worker, family: &str);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Slot {
    pub uv: [u16; 4],
    pub page: u32,
    pub w: f32,
    pub h: f32,
    pub px: f32,
}

pub(crate) const COORD_FOLLOWERS: &str = "12345";
const COORD_FOLLOWER_COUNT: usize = COORD_FOLLOWERS.len();

#[derive(Debug, Clone, Copy)]
pub(crate) struct SdfGlyph {
    pub slot: Slot,
    pub advance: f32,
    pub kerns: [f32; COORD_FOLLOWER_COUNT],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DigitSlot {
    pub slot: Slot,
    pub advance: f32,
}

pub(crate) type NameKey = (u64, Sym);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MapText {
    Coord,
    Name,
}

type MapKey = (u64, MapText);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Sprite {
    Cursor,
    CursorRed,
}

pub(crate) struct SpriteSheet {
    cursor: ImageBitmap,
    cursor_red: ImageBitmap,
}

async fn decode_png(png: &[u8]) -> Result<ImageBitmap, JsValue> {
    let blob =
        Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&js_sys::Uint8Array::from(png)))?;
    let window = web_sys::window().expect("window");
    JsFuture::from(window.create_image_bitmap_with_blob(&blob)?)
        .await?
        .dyn_into()
}

impl SpriteSheet {
    pub(crate) async fn load() -> Result<Self, JsValue> {
        Ok(Self {
            cursor: decode_png(include_bytes!("assets/cursor.png")).await?,
            cursor_red: decode_png(include_bytes!("assets/cursor_red.png")).await?,
        })
    }

    fn raster(&self, sprite: Sprite) -> Raster {
        let image = match sprite {
            Sprite::Cursor => &self.cursor,
            Sprite::CursorRed => &self.cursor_red,
        };
        let (w, h) = (image.width(), image.height());
        let canvas = OffscreenCanvas::new(w + GUTTER * 2, h + GUTTER * 2).expect("sprite canvas");
        let ctx: OffscreenCanvasRenderingContext2d = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .expect("2d context")
            .unchecked_into();
        ctx.draw_image_with_image_bitmap(image, f64::from(GUTTER), f64::from(GUTTER))
            .expect("draw sprite");
        Raster {
            bitmap: canvas.transfer_to_image_bitmap().expect("sprite bitmap"),
            w,
            h,
            glyphs: Vec::new(),
            kerns: [0.0; COORD_FOLLOWER_COUNT],
        }
    }
}

pub(crate) struct Upload {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub bitmap: ImageBitmap,
}

fn fits_on_page(side: u32, w: u32, h: u32) -> bool {
    w <= side && h + SHELF_SLACK <= side
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shelf {
    y: u32,
    h: u32,
    x: u32,
    used: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    oldest_used: u64,
    height: u32,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct Shelves {
    side: u32,
    list: Vec<Shelf>,
    frame: u64,
}

impl Shelves {
    fn new(side: u32, frame: u64) -> Self {
        Self {
            side,
            list: Vec::new(),
            frame,
        }
    }

    fn begin_frame(&mut self, frame: u64) {
        self.frame = frame;
    }

    fn used_height(&self) -> u32 {
        self.list.last().map_or(0, |s| s.y + s.h)
    }

    fn mark_row_used(&mut self, y: u32) {
        let i = self.list.partition_point(|s| s.y <= y);
        if let Some(s) = i.checked_sub(1).and_then(|i| self.list.get_mut(i)) {
            s.used = self.frame;
        }
    }

    fn place_free(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if !fits_on_page(self.side, w, h) {
            return None;
        }
        let frame = self.frame;
        if let Some(s) = self
            .list
            .iter_mut()
            .find(|s| s.h >= h && s.h <= h + SHELF_SLACK * 2 && s.x + w <= self.side)
        {
            let at = (s.x, s.y);
            s.x += w;
            s.used = frame;
            return Some(at);
        }
        let need = h + SHELF_SLACK;
        let top = self.used_height();
        if top + need > self.side {
            return None;
        }
        self.list.push(Shelf {
            y: top,
            h: need,
            x: w,
            used: frame,
        });
        Some((0, top))
    }

    fn is_in_use(&self, s: &Shelf) -> bool {
        s.used + 1 >= self.frame
    }

    fn eviction_candidate(&self, w: u32, h: u32) -> Option<Candidate> {
        if !fits_on_page(self.side, w, h) {
            return None;
        }
        let need = h + SHELF_SLACK;
        let tail = self.side - self.used_height();
        let mut best: Option<Candidate> = None;
        for start in 0..self.list.len() {
            let (mut height, mut oldest_used) = (0, 0);
            for end in start..self.list.len() {
                let s = self.list[end];
                if self.is_in_use(&s) {
                    break;
                }
                height += s.h;
                oldest_used = oldest_used.max(s.used);
                let span = if end + 1 == self.list.len() {
                    height + tail
                } else {
                    height
                };
                if span >= need {
                    if best.is_none_or(|b| (oldest_used, height) < (b.oldest_used, b.height)) {
                        best = Some(Candidate {
                            oldest_used,
                            height,
                            start,
                            end,
                        });
                    }
                    break;
                }
            }
        }
        best
    }

    fn evict(&mut self, c: Candidate, w: u32, h: u32) -> (u32, (u32, u32)) {
        let need = h + SHELF_SLACK;
        let y = self.list[c.start].y;
        let reaches_tail = c.end + 1 == self.list.len();
        let mut fresh = vec![Shelf {
            y,
            h: need,
            x: w,
            used: self.frame,
        }];
        if !reaches_tail && c.height > need {
            fresh.push(Shelf {
                y: y + need,
                h: c.height - need,
                x: 0,
                used: 0,
            });
        }
        self.list.splice(c.start..=c.end, fresh);
        (y, (y, y + c.height))
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    slot: Slot,
    y: u32,
    epoch: u32,
}

#[derive(Debug)]
struct DigitSet {
    slots: Vec<DigitSlot>,
    page: u32,
    y: u32,
    epoch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Job {
    Name(NameKey, usize),
    Circular(NameKey, usize),
    Coord(&'static str, usize),
    CoordSdf(char),
    Map(MapKey, usize),
    Digits(usize),
    Sprite(Sprite),
}

impl Job {
    fn pixel_size(self) -> f32 {
        match self {
            Job::Name(_, t)
            | Job::Circular(_, t)
            | Job::Coord(_, t)
            | Job::Map(_, t)
            | Job::Digits(t) => TIERS[t],
            Job::CoordSdf(_) => SDF_PX,
            Job::Sprite(_) => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GlyphCell {
    x: u32,
    w: u32,
    advance: f32,
}

struct Raster {
    bitmap: ImageBitmap,
    w: u32,
    h: u32,
    glyphs: Vec<GlyphCell>,
    kerns: [f32; COORD_FOLLOWER_COUNT],
}

type Inbox = Rc<RefCell<Vec<(u32, JsValue)>>>;

struct RasterWorker {
    worker: Worker,
    inbox: Inbox,
    failure: Rc<RefCell<Option<String>>>,
    fonts_loaded: Rc<Cell<bool>>,
    _on_result: Closure<dyn FnMut(u32, JsValue)>,
    _on_failure: Closure<dyn FnMut(String)>,
    _on_fonts: Closure<dyn FnMut()>,
}

impl RasterWorker {
    fn spawn() -> Result<Self, JsValue> {
        let inbox: Inbox = Rc::default();
        let failure = Rc::new(RefCell::new(None));
        let sink = Rc::clone(&inbox);
        let on_result = Closure::<dyn FnMut(u32, JsValue)>::new(move |id, item| {
            sink.borrow_mut().push((id, item));
        });
        let reported = Rc::clone(&failure);
        let on_failure = Closure::<dyn FnMut(String)>::new(move |reason: String| {
            reported.borrow_mut().get_or_insert(reason);
        });
        let fonts_loaded = Rc::new(Cell::new(false));
        let loaded = Rc::clone(&fonts_loaded);
        let on_fonts = Closure::<dyn FnMut()>::new(move || loaded.set(true));
        let worker = start_worker(
            on_result.as_ref().unchecked_ref(),
            on_failure.as_ref().unchecked_ref(),
            on_fonts.as_ref().unchecked_ref(),
            &CANVAS_FAMILIES
                .iter()
                .map(|f| JsValue::from_str(f))
                .collect(),
        )
        .map_err(|e| {
            JsValue::from_str(&format!(
                "text worker failed to start: {}",
                super::js_reason(&e)
            ))
        })?;
        Ok(Self {
            worker,
            inbox,
            failure,
            fonts_loaded,
            _on_result: on_result,
            _on_failure: on_failure,
            _on_fonts: on_fonts,
        })
    }
}

impl Drop for RasterWorker {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

pub(crate) struct TextAtlas {
    pages: Vec<Shelves>,
    frame: u64,
    names: HashMap<(NameKey, usize), Entry>,
    coords: HashMap<(&'static str, usize), Entry>,
    coord_sdfs: HashMap<char, (Entry, f32, [f32; COORD_FOLLOWER_COUNT])>,
    sdf_coords: bool,
    map_texts: HashMap<(MapKey, usize), Entry>,
    circular: HashMap<(NameKey, usize), Entry>,
    digits: [Option<DigitSet>; TIERS.len()],
    sprites: HashMap<Sprite, Entry>,
    sheet: SpriteSheet,
    uploads: Vec<Upload>,
    worker: RasterWorker,
    outbox: js_sys::Array,
    pending: HashMap<u32, Job>,
    queued: HashSet<Job>,
    waiting: HashMap<Job, Raster>,
    wanted: HashMap<Job, u64>,
    oversized: HashSet<Job>,
    next_id: u32,
    deterministic: bool,
    generation: u32,
    font_epoch: u32,
    custom_family: String,
    families: Families,
    map_family: String,
    map_families: Families,
    plain_families: Families,
}

struct Families {
    label: String,
    circular: String,
}

impl Families {
    fn new(custom: &str) -> Self {
        if custom.is_empty() {
            return Self {
                label: LABEL_FAMILIES.to_owned(),
                circular: CIRCULAR_FAMILIES.to_owned(),
            };
        }
        let quoted = format!("\"{}\"", custom.replace('\\', "\\\\").replace('"', "\\\""));
        Self {
            label: format!("{quoted}, {LABEL_FAMILIES}"),
            circular: format!("{quoted}, {CIRCULAR_FAMILIES}"),
        }
    }

    fn font(&self, template: &str, px: f32) -> String {
        template
            .replace("{px}", &format!("{px}"))
            .replace("{family}", &self.label)
            .replace("{circular}", &self.circular)
    }
}

fn uv(x: u32, y: u32, w: u32, h: u32) -> [u16; 4] {
    let n = |v: u32| ((v as f32 / ATLAS_SIDE as f32) * f32::from(u16::MAX)).round() as u16;
    [n(x), n(y), n(x + w), n(y + h)]
}

fn nearest_tier<K: Hash + Eq + Copy>(
    map: &HashMap<(K, usize), Entry>,
    key: K,
    tier: usize,
) -> Option<Entry> {
    (0..TIERS.len())
        .filter_map(|t| map.get(&(key, t)).map(|e| (t.abs_diff(tier), *e)))
        .min_by_key(|&(d, _)| d)
        .map(|(_, e)| e)
}

fn set_js_field(obj: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(obj, &key.into(), value).expect("set field on a plain object");
}

fn line_request(
    families: &Families,
    text: &str,
    template: &str,
    px: f32,
    outlined: bool,
    kind: &str,
) -> js_sys::Object {
    let req = js_sys::Object::new();
    set_js_field(&req, "kind", &kind.into());
    set_js_field(&req, "text", &text.into());
    set_js_field(&req, "font", &families.font(template, px).into());
    set_js_field(&req, "px", &px.into());
    set_js_field(&req, "stroke", &(px * (0.015 / 0.13)).into());
    set_js_field(&req, "outlined", &outlined.into());
    set_js_field(&req, "lineHeight", &1.25_f32.into());
    set_js_field(&req, "gutter", &GUTTER.into());
    req
}

fn circular_request(families: &Families, text: &str, tier: usize) -> js_sys::Object {
    let px = TIERS[tier];
    let req = js_sys::Object::new();
    set_js_field(&req, "kind", &"circular".into());
    set_js_field(&req, "text", &text.into());
    set_js_field(
        &req,
        "font",
        &families.font("bold {px}px {circular}", px).into(),
    );
    set_js_field(&req, "px", &px.into());
    set_js_field(&req, "side", &(px * 10.0).ceil().into());
    set_js_field(&req, "stroke", &(px * 0.13).floor().into());
    set_js_field(&req, "radius", &0.8_f64.into());
    set_js_field(&req, "start", &1.5_f64.into());
    set_js_field(&req, "gutter", &GUTTER.into());
    req
}

fn sdf_request(families: &Families, ch: char) -> js_sys::Object {
    let req = js_sys::Object::new();
    set_js_field(&req, "kind", &"sdf".into());
    set_js_field(&req, "text", &ch.to_string().into());
    set_js_field(
        &req,
        "font",
        &families
            .font("{px}px {family}", SDF_PX * SDF_SUPERSAMPLE)
            .into(),
    );
    set_js_field(&req, "px", &SDF_PX.into());
    set_js_field(&req, "supersample", &SDF_SUPERSAMPLE.into());
    set_js_field(
        &req,
        "followers",
        &(if ch.is_ascii_alphabetic() {
            COORD_FOLLOWERS
        } else {
            ""
        })
        .into(),
    );
    set_js_field(&req, "spread", &SDF_SPREAD.into());
    set_js_field(&req, "lineHeight", &1.25_f32.into());
    set_js_field(&req, "gutter", &GUTTER.into());
    req
}

fn number(obj: &JsValue, key: &str) -> f64 {
    js_sys::Reflect::get(obj, &key.into())
        .expect("raster field")
        .as_f64()
        .expect("raster field is a number")
}

fn raster(item: &JsValue) -> Raster {
    let glyphs = js_sys::Reflect::get(item, &"glyphs".into()).expect("raster glyphs");
    let glyphs = if glyphs.is_undefined() {
        Vec::new()
    } else {
        js_sys::Array::from(&glyphs)
            .iter()
            .map(|g| GlyphCell {
                x: number(&g, "x") as u32,
                w: number(&g, "w") as u32,
                advance: number(&g, "advance") as f32,
            })
            .collect()
    };
    let mut kerns = [0.0; COORD_FOLLOWER_COUNT];
    let listed = js_sys::Reflect::get(item, &"kerns".into()).expect("raster kerns");
    if !listed.is_undefined() {
        for (slot, value) in kerns.iter_mut().zip(js_sys::Array::from(&listed).iter()) {
            *slot = value.as_f64().unwrap_or(0.0) as f32;
        }
    }
    Raster {
        bitmap: js_sys::Reflect::get(item, &"bitmap".into())
            .expect("raster bitmap")
            .unchecked_into(),
        w: number(item, "w") as u32,
        h: number(item, "h") as u32,
        glyphs,
        kerns,
    }
}

impl TextAtlas {
    pub(crate) fn new(sheet: SpriteSheet) -> Result<Self, JsValue> {
        Ok(Self {
            pages: Vec::new(),
            frame: 1,
            names: HashMap::new(),
            coords: HashMap::new(),
            coord_sdfs: HashMap::new(),
            sdf_coords: true,
            map_texts: HashMap::new(),
            circular: HashMap::new(),
            digits: Default::default(),
            sprites: HashMap::new(),
            sheet,
            uploads: Vec::new(),
            worker: RasterWorker::spawn()?,
            outbox: js_sys::Array::new(),
            pending: HashMap::new(),
            queued: HashSet::new(),
            waiting: HashMap::new(),
            wanted: HashMap::new(),
            oversized: HashSet::new(),
            next_id: 0,
            deterministic: false,
            generation: 0,
            font_epoch: 0,
            custom_family: String::new(),
            families: Families::new(""),
            map_family: String::new(),
            map_families: Families::new(""),
            plain_families: Families::new(""),
        })
    }

    pub(crate) fn set_family(&mut self, custom: &str) {
        if self.custom_family == custom {
            return;
        }
        custom.clone_into(&mut self.custom_family);
        self.families = Families::new(custom);
        self.reload_fonts();
    }

    pub(crate) fn set_map_family(&mut self, family: &str) {
        if self.map_family == family {
            return;
        }
        family.clone_into(&mut self.map_family);
        self.map_families = Families::new(family);
        self.reload_fonts();
    }

    pub(crate) fn uses_family(&self, family: &str) -> bool {
        self.custom_family == family || self.map_family == family
    }

    pub(crate) fn reload_fonts(&mut self) {
        for family in [&self.custom_family, &self.map_family] {
            if !family.is_empty() {
                add_faces(&self.worker.worker, family);
            }
        }
        self.refresh_fonts();
    }

    fn refresh_fonts(&mut self) {
        for (_, r) in self.waiting.drain() {
            r.bitmap.close();
        }
        self.pending.clear();
        self.queued.clear();
        self.wanted.clear();
        self.oversized.clear();
        self.outbox = js_sys::Array::new();
        self.font_epoch += 1;
    }

    fn is_stale(&self, entry: Option<&Entry>) -> bool {
        entry.is_none_or(|e| e.epoch != self.font_epoch)
    }

    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    pub(crate) fn pages(&self) -> u32 {
        self.pages.len() as u32
    }

    pub(crate) fn pending(&self) -> u32 {
        self.queued.len() as u32
    }

    pub(crate) fn begin_frame(&mut self, deterministic: bool) -> Result<(), JsValue> {
        if let Some(reason) = self
            .worker
            .failure
            .borrow()
            .as_deref()
            .filter(|_| !deterministic)
        {
            return Err(JsValue::from_str(&format!("text worker failed: {reason}")));
        }
        if self.worker.fonts_loaded.replace(false) {
            self.refresh_fonts();
        }
        self.deterministic = deterministic;
        self.frame += 1;
        let frame = self.frame;
        for p in &mut self.pages {
            p.begin_frame(frame);
        }
        let arrived: Vec<(u32, JsValue)> = self.worker.inbox.borrow_mut().drain(..).collect();
        for (id, item) in arrived {
            let r = raster(&item);
            match self.pending.remove(&id) {
                Some(job) => {
                    self.waiting.insert(job, r);
                }
                None => r.bitmap.close(),
            }
        }
        let jobs: Vec<Job> = self.waiting.keys().copied().collect();
        for job in jobs {
            if self.wanted.get(&job).is_some_and(|&f| f + 1 >= frame) {
                self.place_finished_raster(job);
            } else {
                self.drop_waiting(job);
            }
        }
        let queued = &self.queued;
        self.wanted
            .retain(|job, f| *f + 1 >= frame || queued.contains(job));
        Ok(())
    }

    fn drop_waiting(&mut self, job: Job) {
        let r = self.waiting.remove(&job).expect("dropped job is waiting");
        r.bitmap.close();
        self.queued.remove(&job);
        self.wanted.remove(&job);
    }

    pub(crate) fn flush(&mut self) {
        if self.outbox.length() == 0 {
            return;
        }
        let msg = js_sys::Object::new();
        set_js_field(&msg, "jobs", &self.outbox);
        self.worker
            .worker
            .post_message(&msg)
            .expect("text jobs are plain objects");
        self.outbox = js_sys::Array::new();
    }

    pub(crate) fn reset(&mut self) {
        self.pages.clear();
        self.names.clear();
        self.coords.clear();
        self.coord_sdfs.clear();
        self.map_texts.clear();
        self.circular.clear();
        self.digits = Default::default();
        self.sprites.clear();
        for u in self.uploads.drain(..) {
            u.bitmap.close();
        }
        for (_, r) in self.waiting.drain() {
            r.bitmap.close();
        }
        self.pending.clear();
        self.queued.clear();
        self.wanted.clear();
        self.oversized.clear();
        self.outbox = js_sys::Array::new();
        self.generation += 1;
    }

    pub(crate) fn tier_for(px: f32, max_tier: usize) -> usize {
        TIERS
            .iter()
            .position(|&t| t >= px)
            .unwrap_or(TIERS.len() - 1)
            .min(max_tier)
    }

    pub(crate) fn take_uploads(&mut self) -> Vec<Upload> {
        std::mem::take(&mut self.uploads)
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32, u32)> {
        for (page, shelves) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = shelves.place_free(w, h) {
                return Some((page as u32, x, y));
            }
        }
        if (self.pages.len() as u32) < 4 {
            let mut shelves = Shelves::new(ATLAS_SIDE, self.frame);
            let (x, y) = shelves.place_free(w, h).expect("raster fits an empty page");
            self.pages.push(shelves);
            return Some((self.pages.len() as u32 - 1, x, y));
        }
        let (page, candidate) = self
            .pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.eviction_candidate(w, h).map(|c| (i, c)))
            .min_by_key(|(_, c)| (c.oldest_used, c.height))?;
        let (y, (y0, y1)) = self.pages[page].evict(candidate, w, h);
        let page = page as u32;
        let gone = |e: &Entry| e.slot.page == page && (y0..y1).contains(&e.y);
        self.names.retain(|_, e| !gone(e));
        self.coords.retain(|_, e| !gone(e));
        self.coord_sdfs.retain(|_, (e, _, _)| !gone(e));
        self.map_texts.retain(|_, e| !gone(e));
        self.circular.retain(|_, e| !gone(e));
        self.sprites.retain(|_, e| !gone(e));
        for set in &mut self.digits {
            if set
                .as_ref()
                .is_some_and(|s| s.page == page && (y0..y1).contains(&s.y))
            {
                *set = None;
            }
        }
        self.generation += 1;
        Some((page, 0, y))
    }

    fn place_finished_raster(&mut self, job: Job) {
        let r = self.waiting.remove(&job).expect("settled job is waiting");
        let (w, h) = (r.w + GUTTER * 2, r.h + GUTTER * 2);
        if !fits_on_page(ATLAS_SIDE, w, h) {
            r.bitmap.close();
            self.queued.remove(&job);
            self.wanted.remove(&job);
            self.oversized.insert(job);
            return;
        }
        let Some((page, x, y)) = self.allocate(w, h) else {
            self.waiting.insert(job, r);
            return;
        };
        self.queued.remove(&job);
        self.wanted.remove(&job);
        let px = job.pixel_size();
        let (ox, oy) = (x + GUTTER, y + GUTTER);
        let slot = |dx: u32, w: u32| Slot {
            uv: uv(ox + dx, oy, w, r.h),
            page,
            w: w as f32,
            h: r.h as f32,
            px,
        };
        let entry = Entry {
            slot: slot(0, r.w),
            y,
            epoch: self.font_epoch,
        };
        match job {
            Job::Name(key, tier) => {
                self.names.insert((key, tier), entry);
            }
            Job::Circular(key, tier) => {
                self.circular.insert((key, tier), entry);
            }
            Job::Coord(text, tier) => {
                self.coords.insert((text, tier), entry);
            }
            Job::CoordSdf(ch) => {
                let advance = r.glyphs.first().map_or(0.0, |g| g.advance);
                self.coord_sdfs.insert(ch, (entry, advance, r.kerns));
            }
            Job::Map(key, tier) => {
                self.map_texts.insert((key, tier), entry);
            }
            Job::Digits(tier) => {
                let slots = r
                    .glyphs
                    .iter()
                    .map(|g| DigitSlot {
                        slot: slot(g.x, g.w),
                        advance: g.advance,
                    })
                    .collect();
                self.digits[tier] = Some(DigitSet {
                    slots,
                    page,
                    y,
                    epoch: self.font_epoch,
                });
            }
            Job::Sprite(sprite) => {
                self.sprites.insert(sprite, entry);
            }
        }
        self.pages[page as usize].mark_row_used(y);
        self.uploads.push(Upload {
            page,
            x,
            y,
            bitmap: r.bitmap,
        });
    }

    fn request_raster(&mut self, job: Job, req: impl FnOnce(&Families) -> js_sys::Object) {
        if self.oversized.contains(&job) {
            return;
        }
        self.wanted.insert(job, self.frame);
        if self.queued.contains(&job) {
            return;
        }
        let families = match job {
            Job::Map((_, MapText::Coord), _) => &self.plain_families,
            Job::Map(_, _) => &self.map_families,
            _ => &self.families,
        };
        if !self.deterministic {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            let req = req(families);
            set_js_field(&req, "id", &id.into());
            self.outbox.push(&req);
            self.pending.insert(id, job);
            self.queued.insert(job);
            return;
        }
        let r = raster(&render(&req(families)));
        self.queued.insert(job);
        self.waiting.insert(job, r);
        self.place_finished_raster(job);
    }

    fn touch(&mut self, e: Entry) -> Slot {
        self.pages[e.slot.page as usize].mark_row_used(e.y);
        e.slot
    }

    pub(crate) fn name_slot(
        &mut self,
        key: NameKey,
        text: impl FnOnce() -> String,
        tier: usize,
    ) -> Option<Slot> {
        if self.is_stale(self.names.get(&(key, tier))) {
            let px = TIERS[tier];
            self.request_raster(Job::Name(key, tier), |families| {
                line_request(families, &text(), "600 {px}px {family}", px, true, "line")
            });
        }
        let e = match self.names.get(&(key, tier)) {
            Some(&e) => e,
            None => nearest_tier(&self.names, key, tier)?,
        };
        Some(self.touch(e))
    }

    pub(crate) fn circular_name(
        &mut self,
        key: NameKey,
        text: impl FnOnce() -> String,
        tier: usize,
    ) -> Option<Slot> {
        let tier = tier.min(2);
        if self.is_stale(self.circular.get(&(key, tier))) {
            self.request_raster(Job::Circular(key, tier), |families| {
                circular_request(families, &text(), tier)
            });
        }
        let e = match self.circular.get(&(key, tier)) {
            Some(&e) => e,
            None => nearest_tier(&self.circular, key, tier)?,
        };
        Some(self.touch(e))
    }

    pub(crate) fn coord_slot(&mut self, text: &'static str, tier: usize) -> Option<Slot> {
        if self.is_stale(self.coords.get(&(text, tier))) {
            let px = TIERS[tier];
            self.request_raster(Job::Coord(text, tier), |families| {
                line_request(families, text, "{px}px {family}", px, false, "line")
            });
        }
        let e = match self.coords.get(&(text, tier)) {
            Some(&e) => e,
            None => nearest_tier(&self.coords, text, tier)?,
        };
        Some(self.touch(e))
    }

    pub(crate) fn set_sdf_coords(&mut self, on: bool) {
        self.sdf_coords = on;
    }

    pub(crate) fn sdf_coords(&self) -> bool {
        self.sdf_coords
    }

    pub(crate) fn coord_family(&self) -> &str {
        &self.families.label
    }

    pub(crate) fn font_epoch(&self) -> u32 {
        self.font_epoch
    }

    pub(crate) fn coord_sdf(&mut self, ch: char) -> Option<SdfGlyph> {
        let cached = self.coord_sdfs.get(&ch).copied();
        if self.is_stale(cached.as_ref().map(|(e, _, _)| e)) {
            self.request_raster(Job::CoordSdf(ch), |families| sdf_request(families, ch));
        }
        let (e, advance, kerns) = self.coord_sdfs.get(&ch).copied()?;
        Some(SdfGlyph {
            slot: self.touch(e),
            advance,
            kerns,
        })
    }

    pub(crate) fn map_text(&mut self, text: &str, style: MapText, tier: usize) -> Option<Slot> {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let key = (hasher.finish(), style);
        if self.is_stale(self.map_texts.get(&(key, tier))) {
            let px = TIERS[tier];
            self.request_raster(Job::Map(key, tier), |families| {
                line_request(families, text, "{px}px {family}", px, false, "line")
            });
        }
        let e = match self.map_texts.get(&(key, tier)) {
            Some(&e) => e,
            None => nearest_tier(&self.map_texts, key, tier)?,
        };
        Some(self.touch(e))
    }

    pub(crate) fn sprite_slot(&mut self, sprite: Sprite) -> Option<Slot> {
        let job = Job::Sprite(sprite);
        if !self.sprites.contains_key(&sprite) {
            self.wanted.insert(job, self.frame);
            if self.queued.insert(job) {
                self.waiting.insert(job, self.sheet.raster(sprite));
                self.place_finished_raster(job);
            }
        }
        let e = *self.sprites.get(&sprite)?;
        Some(self.touch(e))
    }

    pub(crate) fn digit_slots(&mut self, tier: usize) -> Option<&[DigitSlot]> {
        if self.digits[tier]
            .as_ref()
            .is_none_or(|s| s.epoch != self.font_epoch)
        {
            let px = TIERS[tier];
            self.request_raster(Job::Digits(tier), |families| {
                line_request(families, DIGITS, "bold {px}px {family}", px, true, "glyphs")
            });
        }
        let set = (0..TIERS.len())
            .filter_map(|t| self.digits[t].as_ref().map(|s| (t.abs_diff(tier), s)))
            .min_by_key(|&(d, _)| d)?
            .1;
        self.pages[set.page as usize].mark_row_used(set.y);
        Some(&set.slots)
    }
}
