use bytes::Bytes;
use std::sync::Mutex;
use tokio::sync::Notify;

pub const CLOSE_NO_STATUS: u16 = 1005;
pub const CLOSE_NORMAL: u16 = 1000;
pub const CLOSE_PROTOCOL_ERROR: u16 = 1002;
pub const CLOSE_MESSAGE_TOO_BIG: u16 = 1009;

const MAX_QUEUED_BYTES: usize = 8 << 20;

#[derive(Default)]
pub struct Drained {
    pub frames: Vec<Bytes>,
    pub close: Option<(u16, String)>,
}

#[derive(Default)]
struct Inner {
    drained: Drained,
    closing: bool,
    queued_bytes: usize,
}

#[derive(Default)]
pub struct Outbox {
    inner: Mutex<Inner>,
    notify: Notify,
    closing: Notify,
}

impl Outbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, frame: impl Into<Bytes>) {
        let mut inner = self.inner.lock().unwrap();
        if inner.closing {
            return;
        }
        let frame = frame.into();
        inner.queued_bytes += frame.len();
        if inner.queued_bytes > MAX_QUEUED_BYTES {
            inner.closing = true;
            inner.queued_bytes = 0;
            inner.drained.frames = Vec::new();
            inner.drained.close = Some((CLOSE_NORMAL, "Too slow".to_owned()));
            drop(inner);
            self.notify.notify_one();
            self.closing.notify_one();
            return;
        }
        inner.drained.frames.push(frame);
        drop(inner);
        self.notify.notify_one();
    }

    pub fn is_closing(&self) -> bool {
        self.inner.lock().unwrap().closing
    }

    pub fn close(&self, code: u16, reason: &str) {
        let mut inner = self.inner.lock().unwrap();
        if inner.closing {
            return;
        }
        inner.closing = true;
        inner.drained.close = Some((code, reason.to_owned()));
        drop(inner);
        self.notify.notify_one();
        self.closing.notify_one();
    }

    pub fn drain(&self) -> Drained {
        let mut inner = self.inner.lock().unwrap();
        inner.queued_bytes = 0;
        std::mem::take(&mut inner.drained)
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }

    pub async fn closing(&self) {
        self.closing.notified().await;
    }
}
