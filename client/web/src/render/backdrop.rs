use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    Blob, ImageBitmap, ImageBitmapOptions, PremultiplyAlpha, ReferrerPolicy, RequestCredentials,
    RequestInit, RequestMode, ResizeQuality, Response,
};

const EXTENSIONS: [&str; 4] = [".png", ".jpg", ".gif", ".jpeg"];

fn valid_uri(uri: &str) -> bool {
    let rest = uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"));
    rest.is_some_and(|r| EXTENSIONS.iter().any(|e| r.ends_with(e)))
}

enum State {
    Empty,
    Loading,
    Ready(Vec<ImageBitmap>),
    Failed,
}

type Inbox = Rc<RefCell<Option<(String, Option<Vec<ImageBitmap>>)>>>;

pub(crate) struct Backdrop {
    url: String,
    state: State,
    inbox: Inbox,
    generation: u32,
}

async fn fetch_mips(url: &str) -> Result<Vec<ImageBitmap>, JsValue> {
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
    let premultiplied = ImageBitmapOptions::new();
    premultiplied.set_premultiply_alpha(PremultiplyAlpha::Premultiply);
    let original: ImageBitmap = JsFuture::from(
        window.create_image_bitmap_with_blob_and_image_bitmap_options(&blob, &premultiplied)?,
    )
    .await?
    .dyn_into()?;
    let mut mips = Vec::new();
    let (mut w, mut h) = (original.width(), original.height());
    loop {
        let options = ImageBitmapOptions::new();
        options.set_premultiply_alpha(PremultiplyAlpha::Premultiply);
        options.set_resize_width(w);
        options.set_resize_height(h);
        options.set_resize_quality(ResizeQuality::High);
        let level: ImageBitmap = JsFuture::from(
            window.create_image_bitmap_with_image_bitmap_and_image_bitmap_options(
                &original, &options,
            )?,
        )
        .await?
        .dyn_into()?;
        mips.push(level);
        if w.max(h) / 2 < 64 {
            break;
        }
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    original.close();
    Ok(mips)
}

impl Backdrop {
    pub(crate) fn pending(&self) -> bool {
        matches!(self.state, State::Loading)
    }

    pub(crate) fn new() -> Self {
        Self {
            url: String::new(),
            state: State::Empty,
            inbox: Rc::default(),
            generation: 0,
        }
    }

    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    pub(crate) fn mips(&self) -> Option<&[ImageBitmap]> {
        match &self.state {
            State::Ready(mips) => Some(mips),
            _ => None,
        }
    }

    fn release(&mut self) {
        if let State::Ready(mips) = &self.state {
            mips.iter().for_each(ImageBitmap::close);
        }
        self.state = State::Empty;
        self.generation += 1;
    }

    pub(crate) fn update(&mut self, uri: &str) {
        let arrived = self.inbox.borrow_mut().take();
        if let Some((url, mips)) = arrived {
            if url == self.url {
                self.release();
                self.state = mips.map_or(State::Failed, State::Ready);
                self.generation += 1;
            } else {
                mips.into_iter().flatten().for_each(|b| b.close());
            }
        }
        if uri == self.url {
            return;
        }
        self.release();
        self.url = uri.to_owned();
        if !valid_uri(uri) {
            return;
        }
        self.state = State::Loading;
        let inbox = Rc::clone(&self.inbox);
        let url = uri.to_owned();
        spawn_local(async move {
            let mips = fetch_mips(&url)
                .await
                .inspect_err(|e| super::warn(&format!("background image {url} failed"), e))
                .ok();
            *inbox.borrow_mut() = Some((url, mips));
        });
    }

    pub(crate) fn size(&self) -> Option<(f32, f32)> {
        self.mips()
            .and_then(<[_]>::first)
            .map(|b| (b.width() as f32, b.height() as f32))
    }
}
