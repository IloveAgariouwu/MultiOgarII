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
    /// A view update was pushed and the writer has not taken it yet.
    update_pending: bool,
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
        self.push_frame(frame.into(), false);
    }

    /// Pushes a view update. Until the writer takes it, [`Self::update_pending`]
    /// is true, so the game can leave out the next update instead of queueing
    /// stale ones behind a slow connection.
    pub fn push_update(&self, frame: impl Into<Bytes>) {
        self.push_frame(frame.into(), true);
    }

    /// Whether the last view update is still waiting for the writer. The writer
    /// takes everything as soon as it is woken, so this stays true only while it
    /// is blocked on a connection that does not read fast enough.
    pub fn update_pending(&self) -> bool {
        self.inner.lock().unwrap().update_pending
    }

    fn push_frame(&self, frame: Bytes, is_update: bool) {
        let mut inner = self.inner.lock().unwrap();
        if inner.closing {
            return;
        }
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
        inner.update_pending |= is_update;
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
        inner.update_pending = false;
        std::mem::take(&mut inner.drained)
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }

    pub async fn closing(&self) {
        self.closing.notified().await;
    }
}
