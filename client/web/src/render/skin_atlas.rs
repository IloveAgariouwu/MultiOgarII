use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ogar_core::skin_url::{sanitize_skin_url, MAX_SIDE};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    Blob, ImageBitmap, ImageBitmapOptions, PremultiplyAlpha, ReferrerPolicy, RequestCredentials,
    RequestInit, RequestMode, ResizeQuality, Response,
};

const INITIAL_LAYERS: u32 = 8;
const MAX_LAYERS: u32 = 256;

pub(crate) struct SkinTexture<T> {
    pub(crate) texture: T,
    pub(crate) side: u32,
    pub(crate) capacity: u32,
    pub(crate) generation: u32,
}

pub(crate) fn mip_sides(side: u32) -> Vec<u32> {
    std::iter::successors(Some(side), |&s| (s / 2 >= 16).then_some(s / 2)).collect()
}

fn max_layers(side: u32) -> u32 {
    let layer_bytes: u64 = mip_sides(side)
        .iter()
        .map(|&s| u64::from(s) * u64::from(s) * 4)
        .sum();
    ((96 << 20) / layer_bytes).clamp(u64::from(INITIAL_LAYERS), u64::from(MAX_LAYERS)) as u32
}

enum State {
    Loading,
    Ready(u32),
    Failed,
    Deferred,
}

struct Layer {
    owner: Rc<str>,
    last_used: u64,
}

pub(crate) struct SkinUpload {
    pub layer: u32,
    pub mips: Vec<ImageBitmap>,
}

type Inbox = Rc<RefCell<Vec<(u32, Rc<str>, Option<Vec<ImageBitmap>>)>>>;

pub(crate) struct SkinCache {
    side: u32,
    smooth: bool,
    states: HashMap<Rc<str>, State>,
    layers: Vec<Option<Layer>>,
    capacity: u32,
    inbox: Inbox,
    uploads: Vec<SkinUpload>,
    generation: u32,
    frame: u64,
    free_slots: u32,
}

async fn fetch_bitmap(url: &str) -> Result<ImageBitmap, JsValue> {
    let window = web_sys::window().expect("window");
    let init = RequestInit::new();
    init.set_mode(RequestMode::Cors);
    init.set_credentials(RequestCredentials::Omit);
    init.set_referrer_policy(ReferrerPolicy::NoReferrer);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await?
        .dyn_into()?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()).into());
    }
    let blob: Blob = JsFuture::from(response.blob()?).await?.dyn_into()?;
    let options = ImageBitmapOptions::new();
    options.set_premultiply_alpha(PremultiplyAlpha::Premultiply);
    let bitmap: ImageBitmap = JsFuture::from(
        window.create_image_bitmap_with_blob_and_image_bitmap_options(&blob, &options)?,
    )
    .await?
    .dyn_into()?;
    if bitmap.width() > MAX_SIDE || bitmap.height() > MAX_SIDE {
        bitmap.close();
        return Err("skin too large".into());
    }
    Ok(bitmap)
}

async fn resized(source: &ImageBitmap, side: u32, smooth: bool) -> Result<ImageBitmap, JsValue> {
    let window = web_sys::window().expect("window");
    let options = ImageBitmapOptions::new();
    options.set_premultiply_alpha(PremultiplyAlpha::Premultiply);
    options.set_resize_width(side);
    options.set_resize_height(side);
    options.set_resize_quality(if smooth {
        ResizeQuality::High
    } else {
        ResizeQuality::Low
    });
    JsFuture::from(
        window.create_image_bitmap_with_image_bitmap_and_image_bitmap_options(source, &options)?,
    )
    .await?
    .dyn_into()
}

async fn load(url: String, side: u32, smooth: bool) -> Option<Vec<ImageBitmap>> {
    let clean = sanitize_skin_url(&url);
    if clean.is_empty() {
        super::warn("skin url rejected", &url.into());
        return None;
    }
    let original = match fetch_bitmap(&clean).await {
        Ok(b) => b,
        Err(e) => {
            super::warn(&format!("skin {clean} failed"), &e);
            return None;
        }
    };
    let mut mips = Vec::new();
    for s in mip_sides(side) {
        match resized(&original, s, smooth).await {
            Ok(b) => mips.push(b),
            Err(e) => {
                super::warn(&format!("skin {clean} resize failed"), &e);
                original.close();
                mips.iter().for_each(ImageBitmap::close);
                return None;
            }
        }
    }
    original.close();
    Some(mips)
}

impl SkinCache {
    pub(crate) fn new(side: u32) -> Self {
        Self {
            side,
            smooth: true,
            states: HashMap::new(),
            layers: Vec::new(),
            capacity: INITIAL_LAYERS,
            inbox: Rc::default(),
            uploads: Vec::new(),
            generation: 0,
            frame: 0,
            free_slots: 0,
        }
    }

    pub(crate) fn side(&self) -> u32 {
        self.side
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.capacity
    }

    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    pub(crate) fn set_format(&mut self, side: u32, smooth: bool) {
        if (side, smooth) == (self.side, self.smooth) {
            return;
        }
        self.side = side;
        self.smooth = smooth;
        self.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.layers.clear();
        for upload in self.uploads.drain(..) {
            upload.mips.iter().for_each(ImageBitmap::close);
        }
        self.states.clear();
        self.capacity = INITIAL_LAYERS;
        self.generation += 1;
    }

    pub(crate) fn pending(&self) -> u32 {
        self.states
            .values()
            .filter(|s| matches!(s, State::Loading))
            .count() as u32
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
        let arrived: Vec<_> = self.inbox.borrow_mut().drain(..).collect();
        for (generation, url, loaded) in arrived {
            let current = generation == self.generation;
            let (true, Some(State::Loading)) = (current, self.states.get(&url)) else {
                loaded.into_iter().flatten().for_each(|b| b.close());
                continue;
            };
            match loaded {
                Some(mips) => match self.free_layer() {
                    Some(index) => {
                        self.layers[index as usize] = Some(Layer {
                            owner: Rc::clone(&url),
                            last_used: self.frame,
                        });
                        self.states.insert(url, State::Ready(index));
                        self.uploads.push(SkinUpload { layer: index, mips });
                    }
                    None => {
                        mips.iter().for_each(ImageBitmap::close);
                        self.states.insert(url, State::Deferred);
                    }
                },
                None => {
                    self.states.insert(url, State::Failed);
                }
            }
        }
        let idle = self
            .layers
            .iter()
            .filter(|l| l.as_ref().is_none_or(|l| l.last_used + 1 < self.frame))
            .count() as u32;
        self.free_slots = idle + max_layers(self.side) - self.layers.len() as u32;
    }

    fn free_layer(&mut self) -> Option<u32> {
        if let Some(i) = self.layers.iter().position(Option::is_none) {
            return Some(i as u32);
        }
        if (self.layers.len() as u32) < self.capacity {
            self.layers.push(None);
            return Some(self.layers.len() as u32 - 1);
        }
        let (oldest, last_used) = self
            .layers
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.as_ref().map(|l| (i, l.last_used)))
            .min_by_key(|&(_, used)| used)
            .expect("full layer array");
        if last_used + 1 < self.frame {
            let evicted = self.layers[oldest].take().expect("oldest layer is loaded");
            self.states.remove(&evicted.owner);
            return Some(oldest as u32);
        }
        let limit = max_layers(self.side);
        if self.capacity >= limit {
            return None;
        }
        self.capacity = (self.capacity * 2).min(limit);
        self.layers.push(None);
        Some(self.layers.len() as u32 - 1)
    }

    pub(crate) fn layer_for_url(&mut self, url: &str) -> Option<u32> {
        match self.states.get(url) {
            Some(State::Ready(index)) => {
                let index = *index;
                self.layers[index as usize]
                    .as_mut()
                    .expect("ready skin owns its layer")
                    .last_used = self.frame;
                Some(index)
            }
            Some(State::Loading | State::Failed) => None,
            Some(State::Deferred) if self.free_slots == 0 => None,
            retry @ (Some(State::Deferred) | None) => {
                if retry.is_some() {
                    self.free_slots -= 1;
                }
                let key: Rc<str> = Rc::from(url);
                self.states.insert(Rc::clone(&key), State::Loading);
                let inbox = Rc::clone(&self.inbox);
                let (side, smooth) = (self.side, self.smooth);
                let generation = self.generation;
                spawn_local(async move {
                    let mips = load(key.to_string(), side, smooth).await;
                    inbox.borrow_mut().push((generation, key, mips));
                });
                None
            }
        }
    }

    pub(crate) fn take_uploads(&mut self) -> Vec<SkinUpload> {
        std::mem::take(&mut self.uploads)
    }

    pub(crate) fn all_layers(&self) -> impl Iterator<Item = u32> + '_ {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_some())
            .map(|(i, _)| i as u32)
    }
}
