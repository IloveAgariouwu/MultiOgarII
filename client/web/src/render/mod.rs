mod backdrop;
mod camera;
mod canvas2d;
mod gl;
mod gpu;
mod jelly;
mod look;
mod marks;
mod minimap;
mod motion;
mod options;
mod overlap;
mod primitive;
mod rainbow;
mod scene;
mod skin_atlas;
mod text;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, FontFaceSet, FontFaceSetLoadEvent, HtmlCanvasElement, ResizeObserver,
    ResizeObserverEntry, ResizeObserverSize,
};

use crate::world::WorldHandle;
use crate::Core;
use backdrop::Backdrop;
use camera::{Camera, ReplayCamera, Sight, Stage};
use canvas2d::Canvas2dBackend;
use gl::GlBackend;
use gpu::GpuBackend;
use minimap::{Minimap, Place, Style, View};
use motion::Motion;
use ogar_core::world::{Tab, World};
use options::{Options, Overrides, Rgba};
use overlap::OverlapMark;
use primitive::Prim;
use scene::Scene;
use skin_atlas::SkinCache;
use text::{SpriteSheet, TextAtlas, LABEL_FAMILY, PRELOAD_FONTS};

pub(crate) struct FrameView {
    pub width: u32,
    pub height: u32,
    pub cam_x: f32,
    pub cam_y: f32,
    pub scale: f32,
    pub background: Rgba,
}

impl FrameView {
    pub(crate) fn clear_color(&self) -> [f64; 4] {
        self.background.map(|c| f64::from(c) / 255.0)
    }

    pub(crate) fn screen(&self) -> FrameView {
        FrameView {
            cam_x: self.width as f32 / 2.0,
            cam_y: self.height as f32 / 2.0,
            scale: 1.0,
            ..*self
        }
    }
}

const FRAME_MS: f64 = 1000.0 / 60.0;
const CONTEXT_LOST: &str = "webglcontextlost";
const CONTEXT_RESTORED: &str = "webglcontextrestored";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Under,
    Border,
    Shadows,
    Food,
    Dots,
    Cells,
    Over,
    Minimap,
    MinimapClip,
}

impl Layer {
    pub(crate) const COUNT: usize = 9;

    pub(crate) fn screen_blend(self) -> bool {
        self == Layer::Border
    }

    pub(crate) fn screen_space(self) -> bool {
        matches!(self, Layer::Minimap | Layer::MinimapClip)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenClip {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub(crate) struct Layers<'a> {
    pub under: &'a [Prim],
    pub border: &'a [Prim],
    pub shadows: &'a [Prim],
    pub food: &'a [Prim],
    pub dots: &'a [Prim],
    pub cells: &'a [Prim],
    pub overlaps: &'a [OverlapMark],
    pub jelly: &'a [f32],
    pub over: &'a [Prim],
    pub minimap: &'a [Prim],
    pub minimap_clip: &'a [Prim],
    pub clip: Option<ScreenClip>,
}

impl<'a> Layers<'a> {
    pub(crate) fn prim_lists(&self) -> [(Layer, &'a [Prim]); Layer::COUNT] {
        [
            (Layer::Under, self.under),
            (Layer::Border, self.border),
            (Layer::Shadows, self.shadows),
            (Layer::Food, self.food),
            (Layer::Dots, self.dots),
            (Layer::Cells, self.cells),
            (Layer::Over, self.over),
            (Layer::Minimap, self.minimap),
            (Layer::MinimapClip, self.minimap_clip),
        ]
    }
}

enum Backend {
    Canvas2d(Canvas2dBackend),
    WebGl2(GlBackend),
    WebGpu(GpuBackend),
}

impl Backend {
    fn kind(&self) -> BackendKind {
        match self {
            Backend::Canvas2d(_) => BackendKind::Canvas2d,
            Backend::WebGl2(_) => BackendKind::WebGl2,
            Backend::WebGpu(_) => BackendKind::WebGpu,
        }
    }

    fn max_dimension(&self) -> u32 {
        match self {
            Backend::Canvas2d(_) => u32::MAX,
            Backend::WebGl2(b) => b.max_dimension(),
            Backend::WebGpu(b) => b.max_dimension(),
        }
    }

    fn drawable(&self, canvas: &HtmlCanvasElement) -> (u32, u32) {
        match self {
            Backend::WebGl2(b) => b.drawable(),
            _ => (canvas.width(), canvas.height()),
        }
    }

    async fn create(
        canvas: &HtmlCanvasElement,
        mode: u32,
        state: &Rc<Cell<ContextState>>,
    ) -> Result<Backend, JsValue> {
        let chain = BackendKind::order(mode, gl::renders_in_software);
        let mut failures = Vec::with_capacity(chain.len());
        for &kind in chain {
            match kind.start(canvas, state).await {
                Ok(b) => return Ok(b),
                Err(e) => {
                    let failure = format!("{} backend failed: {}", kind.name(), js_reason(&e));
                    error(&failure);
                    failures.push(failure);
                }
            }
        }
        Err(failures.join("; ").into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendKind {
    WebGl2,
    WebGpu,
    Canvas2d,
}

impl BackendKind {
    const MODE_AUTO: u32 = 0;
    const MODE_WEBGL2: u32 = 1;
    const MODE_WEBGPU: u32 = 2;
    const MODE_CANVAS2D: u32 = 3;

    fn order(mode: u32, renders_in_software: impl Fn() -> bool) -> &'static [BackendKind] {
        match mode {
            Self::MODE_AUTO if renders_in_software() => &[Self::Canvas2d],
            Self::MODE_AUTO => &[Self::WebGl2, Self::WebGpu, Self::Canvas2d],
            Self::MODE_WEBGL2 => &[Self::WebGl2],
            Self::MODE_WEBGPU => &[Self::WebGpu],
            Self::MODE_CANVAS2D => &[Self::Canvas2d],
            other => unreachable!(
                "GpuCellRenderer {other} is outside the 0..=3 range core settings clamp to"
            ),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Canvas2d => "canvas2d",
            Self::WebGl2 => "webgl2",
            Self::WebGpu => "webgpu",
        }
    }

    async fn start(
        self,
        canvas: &HtmlCanvasElement,
        state: &Rc<Cell<ContextState>>,
    ) -> Result<Backend, JsValue> {
        match self {
            Self::WebGpu => GpuBackend::new(canvas, gpu::adapter().await?, state)
                .await
                .map(Backend::WebGpu),
            Self::WebGl2 => GlBackend::new(canvas).map(Backend::WebGl2),
            Self::Canvas2d => Canvas2dBackend::new(canvas).map(Backend::Canvas2d),
        }
    }
}

pub(crate) fn js_reason(e: &JsValue) -> String {
    match (e.dyn_ref::<js_sys::Error>(), e.as_string()) {
        (Some(err), _) => String::from(err.message()),
        (None, Some(text)) => text,
        (None, None) => format!("{e:?}"),
    }
}

#[wasm_bindgen]
pub struct Renderer {
    canvas: HtmlCanvasElement,
    backend: Backend,
    options: Option<Options>,
    core_revision: Option<u32>,
    overrides: Overrides,
    read_only: bool,
    scene: Scene,
    atlas: TextAtlas,
    skins: SkinCache,
    backdrop: Backdrop,
    sight: Sight,
    world_epoch: Option<(u64, u64)>,
    motion: Motion,
    last_frame: f64,
    work_ms: f64,
    fixed_work_ms: Option<f64>,
    overlays: bool,
    started: f64,
    spawns: SpawnWatch,
    pointer: Option<(f32, f32)>,
    camera: Camera,
    css_ratio: f32,
    max_area: Option<u64>,
    fixed_size: Option<(u32, u32)>,
    fonts_loaded: Rc<RefCell<Vec<String>>>,
    fonts: FontWatch,
    context_state: Rc<Cell<ContextState>>,
    gpu_replacement: Rc<RefCell<Option<GpuBackend>>>,
    _context_watch: ContextWatch,
    css_box: Rc<Cell<BoxSize>>,
    _size_watch: SizeWatch,
    minimap: Minimap,
    minimap_only: bool,
}

#[wasm_bindgen]
impl Renderer {
    pub async fn create(canvas: HtmlCanvasElement, mode: u32) -> Result<Renderer, JsValue> {
        let context_state = Rc::new(Cell::new(ContextState::Live));
        let backend = Backend::create(&canvas, mode, &context_state).await?;
        let mut atlas = TextAtlas::new(SpriteSheet::load().await?)?;
        atlas.set_sdf_coords(!matches!(backend, Backend::Canvas2d(_)));
        let fonts_loaded = Rc::default();
        let fonts = watch_fonts(&fonts_loaded);
        let context_watch = ContextWatch::new(&canvas, &context_state);
        let css_box = Rc::new(Cell::new(BoxSize {
            css: (canvas.client_width() as f32, canvas.client_height() as f32),
            device: None,
        }));
        let size_watch = SizeWatch::new(&canvas, &css_box);
        Ok(Renderer {
            canvas,
            backend,
            options: None,
            core_revision: None,
            overrides: Overrides::new(),
            read_only: false,
            scene: Scene::new(),
            atlas,
            skins: SkinCache::new(0),
            backdrop: Backdrop::new(),
            sight: Sight::new(),
            world_epoch: None,
            motion: Motion::new(),
            last_frame: 0.0,
            work_ms: 0.0,
            fixed_work_ms: None,
            overlays: true,
            started: 0.0,
            spawns: SpawnWatch::default(),
            pointer: None,
            camera: Camera {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                css_scale: 1.0,
                half_view: (0.0, 0.0),
                aim: None,
            },
            css_ratio: 1.0,
            max_area: None,
            fixed_size: None,
            fonts_loaded,
            fonts,
            _context_watch: context_watch,
            context_state,
            gpu_replacement: Rc::new(RefCell::new(None)),
            css_box,
            _size_watch: size_watch,
            minimap: Minimap::default(),
            minimap_only: false,
        })
    }

    #[wasm_bindgen(js_name = setMinimapOnly)]
    pub fn set_minimap_only(&mut self, on: bool) {
        self.minimap_only = on;
    }

    #[wasm_bindgen(js_name = setMinimapPlace)]
    pub fn set_minimap_place(&mut self, x: f32, y: f32, side: f32) {
        self.minimap.set_place(Some(Place { x, y, side }));
    }

    #[wasm_bindgen(js_name = clearMinimap)]
    pub fn clear_minimap(&mut self) {
        self.minimap.set_place(None);
    }

    #[wasm_bindgen(js_name = setMinimapStyle)]
    pub fn set_minimap_style(
        &mut self,
        text: f32,
        shrink: f32,
        family: String,
        watching: bool,
        live_view: bool,
    ) {
        self.minimap.set_style(
            Style {
                text,
                shrink,
                watching,
                live_view,
            },
            family,
        );
    }

    #[wasm_bindgen(js_name = setMinimapView)]
    pub fn set_minimap_view(&mut self, view: Option<Vec<f32>>) {
        self.minimap
            .set_view(view.as_deref().and_then(View::from_slice));
    }

    #[wasm_bindgen(js_name = setMinimapCells)]
    pub fn set_minimap_cells(&mut self, cells: Option<Vec<f64>>) {
        self.minimap.set_cells(cells);
    }

    #[wasm_bindgen(js_name = setMinimapPlayers)]
    pub fn set_minimap_players(&mut self, nums: &[f64], texts: Vec<String>) {
        self.minimap.set_players(nums, texts);
    }

    #[wasm_bindgen(js_name = backendName)]
    pub fn backend_name(&self) -> String {
        self.backend.kind().name().to_owned()
    }

    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f32) {
        self.sight.set_scale(zoom);
    }

    #[wasm_bindgen(js_name = resetCamera)]
    pub fn reset_camera(&mut self) {
        self.sight = Sight::new();
    }

    #[wasm_bindgen(js_name = setPointer)]
    pub fn set_pointer(&mut self, css_x: f32, css_y: f32) {
        self.pointer = Some((css_x, css_y));
    }

    #[wasm_bindgen(js_name = clearPointer)]
    pub fn clear_pointer(&mut self) {
        self.pointer = None;
    }

    #[wasm_bindgen(js_name = onWheel)]
    pub fn on_wheel(&mut self, notches: f32) {
        let speed = self.options.as_ref().map_or(1.0, |o| o.zoom_speed);
        self.sight.zoom_by_wheel(notches, speed);
    }

    #[wasm_bindgen(js_name = onPinch)]
    pub fn on_pinch(&mut self, ratio: f32) {
        self.sight.zoom_by(ratio);
    }

    #[wasm_bindgen(js_name = setOverride)]
    pub fn set_override(&mut self, key: &str, json: &str) -> Result<(), JsValue> {
        let value = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("override {key}: {e}")))?;
        self.overrides.insert(key.to_owned(), value);
        self.core_revision = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = setReadOnly)]
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    #[wasm_bindgen(js_name = clearOverrides)]
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
        self.core_revision = None;
    }

    #[wasm_bindgen(js_name = setOverlays)]
    pub fn set_overlays(&mut self, on: bool) {
        self.overlays = on;
        self.core_revision = None;
    }

    #[wasm_bindgen(js_name = cameraState)]
    pub fn camera_state(&self) -> Vec<f64> {
        let (x, y, scale) = self.sight.position_and_scale();
        let (ax, ay) = self.camera.aim.unwrap_or((f32::NAN, f32::NAN));
        [x, y, scale, ax, ay].map(f64::from).to_vec()
    }

    #[wasm_bindgen(js_name = setReplayCamera)]
    pub fn set_replay_camera(&mut self, x: f32, y: f32, scale: Option<f32>) {
        self.sight.set_replay(Some(ReplayCamera { x, y, scale }));
    }

    #[wasm_bindgen(js_name = setStage)]
    pub fn set_stage(&mut self, x: f32, y: f32, scale: f32) {
        self.sight.set_stage(Some(Stage { x, y, scale }));
    }

    #[wasm_bindgen(js_name = clearReplayCamera)]
    pub fn clear_replay_camera(&mut self) {
        self.sight.set_replay(None);
    }

    #[wasm_bindgen(js_name = setFit)]
    pub fn set_fit(&mut self, margin: Option<f32>) {
        self.sight.set_fit(margin);
    }

    #[wasm_bindgen(js_name = setFixedWorkMs)]
    pub fn set_fixed_work_ms(&mut self, ms: Option<f64>) {
        self.fixed_work_ms = ms;
    }

    #[wasm_bindgen(js_name = pendingAssets)]
    pub fn pending_assets(&self) -> u32 {
        let fonts_loading = js_sys::Reflect::get(&self.fonts.fonts, &"status".into())
            .expect("FontFaceSet has a status")
            .as_string()
            .is_some_and(|s| s == "loading");
        self.skins.pending()
            + self.atlas.pending()
            + u32::from(self.backdrop.pending())
            + u32::from(fonts_loading)
    }

    #[wasm_bindgen(js_name = setFixedSize)]
    pub fn set_fixed_size(&mut self, width: u32, height: u32) {
        self.fixed_size = (width > 0 && height > 0).then_some((width, height));
    }

    #[wasm_bindgen(js_name = screenToWorld)]
    pub fn screen_to_world(&self, css_x: f32, css_y: f32) -> Vec<f32> {
        let (x, y) = self.sight.screen_to_world(css_x, css_y, self.css_size());
        vec![x, y]
    }

    fn refresh_options(&mut self, core: &Core) {
        let (revision, settings, colors) = core.render_inputs();
        if self.core_revision == Some(revision) {
            return;
        }
        let o = Options::read(settings, colors, &self.overrides);
        let o = if self.overlays { o } else { o.world_only() };
        self.skins.set_format(o.skin_side, o.smooth_skin);
        self.scene.invalidate_food();
        self.core_revision = Some(revision);
        self.options = Some(o);
    }

    fn check_fonts(&mut self) {
        let loaded = std::mem::take(&mut *self.fonts_loaded.borrow_mut());
        if loaded
            .iter()
            .any(|f| f == LABEL_FAMILY || self.atlas.uses_family(f))
        {
            self.atlas.reload_fonts();
        }
    }

    fn css_size(&self) -> (f32, f32) {
        match self.fixed_size {
            Some((w, h)) => (w as f32, h as f32),
            None => self.css_box.get().css,
        }
    }

    fn resize(&mut self, render_scale: f32) {
        let measured = self.css_box.get();
        let device_ratio = match (self.fixed_size, measured.device) {
            (Some(_), _) => 1.0,
            (None, Some((dw, _))) if measured.css.0 > 0.0 => dw / measured.css.0,
            (None, _) => web_sys::window().expect("window").device_pixel_ratio() as f32,
        };
        let wanted = match self.fixed_size {
            Some(_) => 1.0,
            None => device_ratio * render_scale,
        };
        let (css_w, css_h) = self.css_size();
        let max = self.backend.max_dimension();
        let mut ratio = wanted.min(max as f32 / css_w).min(max as f32 / css_h);
        if let Some(area) = self.max_area {
            ratio = ratio.min((area as f32 / (css_w * css_h)).sqrt());
        }
        let size = |ratio: f32| {
            (
                ((css_w * ratio).round() as u32).clamp(1, max),
                ((css_h * ratio).round() as u32).clamp(1, max),
            )
        };
        let (mut w, mut h) = size(ratio);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
            let (dw, dh) = self.backend.drawable(&self.canvas);
            if dw < w || dh < h {
                self.max_area = Some(u64::from(dw) * u64::from(dh));
                ratio *= (dw as f32 / w as f32).min(dh as f32 / h as f32);
                (w, h) = size(ratio);
                self.canvas.set_width(w);
                self.canvas.set_height(h);
            }
        }
        self.css_ratio = ratio;
    }

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(
        &mut self,
        now: f64,
        world: &mut WorldHandle,
        core: &Core,
    ) -> Result<(), JsValue> {
        self.prepare_frame(now, world, core);
        self.draw(world)
    }

    #[wasm_bindgen(js_name = prepareFrame)]
    pub fn prepare_frame(&mut self, now: f64, world: &mut WorldHandle, core: &Core) {
        self.started = perf_now();
        self.refresh_options(core);
        self.check_fonts();
        let render_scale = self.options.as_ref().expect("options").render_scale;
        self.resize(render_scale);
        let o = self.options.as_ref().expect("options");
        let frame_ms = if self.last_frame > 0.0 {
            now - self.last_frame
        } else {
            0.0
        };
        self.last_frame = now;
        self.scene.advance_rainbow(now, o);
        let lerp = self.motion.begin_frame(
            o,
            now,
            frame_ms,
            self.fixed_work_ms.unwrap_or(self.work_ms),
            &world.inner,
        );
        if !self.read_only {
            world.inner.set_lerp_ms(lerp);
        }
        let spawned =
            self.spawns
                .detect_new_units(&mut world.inner, o.use_spawn_signal, self.read_only);
        let world = &world.inner;
        let epoch = Some((world.id(), world.epoch()));
        if self.world_epoch != epoch {
            self.world_epoch = epoch;
            self.sight.reset_for_new_world();
        }
        if spawned {
            self.sight.on_spawned();
        }
        let pointer = self
            .pointer
            .map(|(x, y)| self.sight.screen_to_world(x, y, self.css_size()));
        self.sight.set_exact(self.fixed_work_ms.is_some());
        let view = self.css_size();
        self.sight.follow_frame(world, now, o, pointer, view);
        let aim = self
            .pointer
            .map(|(x, y)| self.sight.screen_to_world(x, y, self.css_size()));
        self.camera = Camera {
            aim,
            ..self.sight.camera_for_view(self.css_ratio, view)
        };
    }

    fn rebuild_gpu(&mut self) {
        self.context_state.set(ContextState::Rebuilding);
        let canvas = self.canvas.clone();
        let state = Rc::clone(&self.context_state);
        let slot = Rc::clone(&self.gpu_replacement);
        wasm_bindgen_futures::spawn_local(async move {
            let rebuilt = async { GpuBackend::new(&canvas, gpu::adapter().await?, &state).await };
            match rebuilt.await {
                Ok(backend) => {
                    *slot.borrow_mut() = Some(backend);
                    state.set(ContextState::Restored);
                }
                Err(e) => {
                    error(&format!(
                        "WebGPU rebuild failed; rendering stopped: {}",
                        js_reason(&e)
                    ));
                    state.set(ContextState::Dead);
                }
            }
        });
    }

    pub fn draw(&mut self, world: &WorldHandle) -> Result<(), JsValue> {
        match self.context_state.get() {
            ContextState::Lost => {
                if let Backend::WebGpu(_) = self.backend {
                    self.rebuild_gpu();
                }
                return Ok(());
            }
            ContextState::Rebuilding => return Ok(()),
            ContextState::Dead => {
                return Err(
                    "WebGPU device was lost and could not be rebuilt; rendering stopped".into(),
                )
            }
            ContextState::Restored => {
                match &mut self.backend {
                    Backend::WebGpu(old) => {
                        old.release_context();
                        let fresh = self
                            .gpu_replacement
                            .borrow_mut()
                            .take()
                            .expect("rebuilt backend");
                        self.backend = Backend::WebGpu(fresh);
                    }
                    Backend::WebGl2(old) => {
                        old.release_context();
                        self.backend = Backend::WebGl2(GlBackend::new(&self.canvas)?);
                    }
                    Backend::Canvas2d(_) => {}
                }
                self.atlas.reset();
                self.skins.clear();
                self.scene.invalidate_food();
                self.context_state.set(ContextState::Live);
            }
            ContextState::Live => {}
        }
        let o = self.options.as_ref().expect("options");
        let world = &world.inner;
        let (width, height) = (self.canvas.width(), self.canvas.height());
        self.atlas.set_family(&o.cell_font);
        if let Some(family) = self.minimap.font_family(&o.minimap) {
            self.atlas.set_map_family(family);
        }
        self.atlas.begin_frame(self.fixed_work_ms.is_some())?;
        self.skins.begin_frame();
        self.backdrop.update(&o.back_image_uri);
        let food_changed = !self.minimap_only && {
            self.scene
                .build_under(world, o, self.camera, &mut self.atlas, &self.backdrop);
            let changed = self.scene.build_food(world, o, &mut self.motion);
            self.scene.build_cells(
                world,
                o,
                self.camera,
                &mut self.motion,
                &mut self.skins,
                &mut self.atlas,
            );
            changed
        };
        self.minimap.build(
            world,
            &o.minimap,
            &self.camera,
            self.css_ratio,
            self.last_frame,
            &mut self.atlas,
        );
        self.atlas.flush();
        let view = FrameView {
            width,
            height,
            cam_x: self.camera.x,
            cam_y: self.camera.y,
            scale: self.camera.scale,
            background: if self.minimap_only {
                [0; 4]
            } else {
                look::opaque(o.background)
            },
        };
        let layers = Layers {
            under: &self.scene.under,
            border: &self.scene.border,
            shadows: &self.scene.shadows,
            food: &self.scene.food,
            dots: &self.scene.dots,
            cells: &self.scene.cells,
            overlaps: self.scene.overlaps.marks(),
            jelly: &self.scene.jelly_rows,
            over: &self.scene.over,
            minimap: &self.minimap.base,
            minimap_clip: &self.minimap.clipped,
            clip: self.minimap.clip,
        };
        let drawn = match &mut self.backend {
            Backend::Canvas2d(b) => b.draw(
                &view,
                &layers,
                food_changed,
                &mut self.atlas,
                &mut self.skins,
                &self.backdrop,
            ),
            Backend::WebGl2(b) => b.draw(
                &view,
                &layers,
                food_changed,
                &mut self.atlas,
                &mut self.skins,
                &self.backdrop,
            ),
            Backend::WebGpu(b) => b.draw(
                &view,
                &layers,
                food_changed,
                &mut self.atlas,
                &mut self.skins,
                &self.backdrop,
            ),
        };
        self.motion.end_frame();
        let work = perf_now() - self.started;
        self.work_ms = (self.work_ms + work) / 2.0;
        drawn
    }
}

#[derive(Default)]
struct SpawnWatch {
    primed: bool,
    owners: [Vec<u32>; 2],
    present: Vec<u32>,
    waves: Vec<(f32, f32)>,
}

impl SpawnWatch {
    fn detect_new_units(&mut self, world: &mut World, enabled: bool, read_only: bool) -> bool {
        if read_only {
            let [main, second] = &mut self.owners;
            for owner in second.iter() {
                if !main.contains(owner) {
                    main.push(*owner);
                }
            }
            second.clone_from(main);
        }
        let enabled = enabled && (self.primed || !read_only);
        self.wave_new_units(world, enabled) && !read_only
    }

    fn wave_new_units(&mut self, world: &mut World, enabled: bool) -> bool {
        self.waves.clear();
        let mut spawned = false;
        for (tab, seen) in [Tab::Main, Tab::Second].into_iter().zip(&mut self.owners) {
            self.present.clear();
            for cell in world.own(tab).iter().filter_map(|&id| world.cell(id)) {
                if !self.present.contains(&cell.owner) {
                    self.present.push(cell.owner);
                }
            }
            spawned |= self.present.iter().any(|o| !seen.contains(o));
            if enabled {
                for owner in self.present.iter().filter(|o| !seen.contains(o)) {
                    let (x, y, m) = world
                        .own(tab)
                        .iter()
                        .filter_map(|&id| world.cell(id))
                        .filter(|c| c.owner == *owner)
                        .fold((0.0, 0.0, 0.0), |(x, y, m), c| {
                            let mass = c.to.mass();
                            (x + c.to.x * mass, y + c.to.y * mass, m + mass)
                        });
                    self.waves.push((x / m, y / m));
                }
            }
            std::mem::swap(seen, &mut self.present);
        }
        for &(x, y) in &self.waves {
            world.push_wave(x, y);
        }
        self.primed = true;
        spawned
    }
}

fn perf_now() -> f64 {
    web_sys::window()
        .expect("window")
        .performance()
        .expect("performance")
        .now()
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn warn(message: &str, detail: &JsValue);
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn error(message: &str);
}

struct FontWatch {
    fonts: FontFaceSet,
    listener: Closure<dyn FnMut(FontFaceSetLoadEvent)>,
}

fn watch_fonts(loaded: &Rc<RefCell<Vec<String>>>) -> FontWatch {
    let fonts = web_sys::window()
        .expect("window")
        .document()
        .expect("document")
        .fonts();
    let sink = Rc::clone(loaded);
    let listener =
        Closure::<dyn FnMut(FontFaceSetLoadEvent)>::new(move |e: FontFaceSetLoadEvent| {
            sink.borrow_mut().extend(e.fontfaces().iter().map(|f| {
                f.unchecked_into::<web_sys::FontFace>()
                    .family()
                    .trim_matches(['"', '\''])
                    .to_owned()
            }));
        });
    fonts
        .add_event_listener_with_callback("loadingdone", listener.as_ref().unchecked_ref())
        .expect("add loadingdone listener");
    for font in PRELOAD_FONTS {
        let loading = JsFuture::from(fonts.load(font));
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = loading.await {
                warn(&format!("font {font} failed to load"), &e);
            }
        });
    }
    FontWatch { fonts, listener }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.skins.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextState {
    Live,
    Lost,
    Rebuilding,
    Restored,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BoxSize {
    css: (f32, f32),
    device: Option<(f32, f32)>,
}

struct SizeWatch {
    observer: ResizeObserver,
    _callback: Closure<dyn FnMut(js_sys::Array)>,
}

fn first_size(entry: &ResizeObserverEntry, key: &str) -> Option<(f32, f32)> {
    let sizes: js_sys::Array = js_sys::Reflect::get(entry, &key.into())
        .expect("ResizeObserverEntry is an object")
        .dyn_into()
        .ok()?;
    let s: ResizeObserverSize = sizes.get(0).unchecked_into();
    Some((s.inline_size() as f32, s.block_size() as f32))
}

impl SizeWatch {
    fn new(canvas: &HtmlCanvasElement, size: &Rc<Cell<BoxSize>>) -> Self {
        let sink = Rc::clone(size);
        let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let entry: ResizeObserverEntry = entries.get(entries.length() - 1).unchecked_into();
            if let Some(css) = first_size(&entry, "contentBoxSize") {
                sink.set(BoxSize {
                    css,
                    device: first_size(&entry, "devicePixelContentBoxSize"),
                });
            }
        });
        let observer =
            ResizeObserver::new(callback.as_ref().unchecked_ref()).expect("ResizeObserver");
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"box".into(), &"device-pixel-content-box".into())
            .expect("set field on a plain object");
        let observed: js_sys::Function = js_sys::Reflect::get(&observer, &"observe".into())
            .expect("ResizeObserver has observe")
            .unchecked_into();
        if observed.call2(&observer, canvas, &options).is_err() {
            observer.observe(canvas);
        }
        Self {
            observer,
            _callback: callback,
        }
    }
}

impl Drop for SizeWatch {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

impl Drop for FontWatch {
    fn drop(&mut self) {
        self.fonts
            .remove_event_listener_with_callback(
                "loadingdone",
                self.listener.as_ref().unchecked_ref(),
            )
            .expect("remove loadingdone listener");
    }
}

struct ContextWatch {
    canvas: HtmlCanvasElement,
    lost: Closure<dyn FnMut(Event)>,
    restored: Closure<dyn FnMut(Event)>,
}

impl ContextWatch {
    fn new(canvas: &HtmlCanvasElement, state: &Rc<Cell<ContextState>>) -> Self {
        let on_lost = Rc::clone(state);
        let lost = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
            e.prevent_default();
            on_lost.set(ContextState::Lost);
        });
        let on_restored = Rc::clone(state);
        let restored = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            on_restored.set(ContextState::Restored);
        });
        canvas
            .add_event_listener_with_callback(CONTEXT_LOST, lost.as_ref().unchecked_ref())
            .expect("add context lost listener");
        canvas
            .add_event_listener_with_callback(CONTEXT_RESTORED, restored.as_ref().unchecked_ref())
            .expect("add context restored listener");
        Self {
            canvas: canvas.clone(),
            lost,
            restored,
        }
    }
}

impl Drop for ContextWatch {
    fn drop(&mut self) {
        self.canvas
            .remove_event_listener_with_callback(CONTEXT_LOST, self.lost.as_ref().unchecked_ref())
            .expect("remove context lost listener");
        self.canvas
            .remove_event_listener_with_callback(
                CONTEXT_RESTORED,
                self.restored.as_ref().unchecked_ref(),
            )
            .expect("remove context restored listener");
    }
}
