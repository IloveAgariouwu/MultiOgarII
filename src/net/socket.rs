use super::decode::{ClientMsg, Decoded, Decoder};
use bytes::Bytes;
use super::outbox::{
    Drained, Outbox, CLOSE_MESSAGE_TOO_BIG, CLOSE_NO_STATUS, CLOSE_PROTOCOL_ERROR,
};
use crate::world::ConnInfo;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request, Response,
};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Error, Message};

const MAX_PAYLOAD: usize = 4096;
const MAX_MESSAGE: usize = 256;
const READ_BUFFER: usize = 4096;
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(60);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_MESSAGES_PER_SECOND: u32 = 500;
const STAT_SPACING: Duration = Duration::from_secs(1);

pub struct MessageBudget {
    window: Instant,
    used: u32,
}

impl MessageBudget {
    pub fn new(now: Instant) -> Self {
        Self { window: now, used: 0 }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        if now.duration_since(self.window) >= Duration::from_secs(1) {
            self.window = now;
            self.used = 0;
        }
        self.used += 1;
        self.used <= MAX_MESSAGES_PER_SECOND
    }
}

pub enum Event {
    Open { id: u64, info: ConnInfo },
    Message { id: u64, msg: ClientMsg },
    Closed { id: u64 },
}

enum Ended {
    ProtocolError(u16),
    Quiet,
    Expired,
}

#[derive(Default)]
struct UpgradeHeaders {
    origin: Option<String>,
    user_agent: Option<String>,
    cache_control: Option<String>,
}

fn header(request: &Request, name: &str) -> Option<String> {
    request.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

fn close_frame(code: u16, reason: String) -> Message {
    if code == CLOSE_NO_STATUS {
        return Message::Close(None);
    }
    Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.into() }))
}

#[allow(clippy::result_large_err)]
pub async fn serve<S>(
    stream: S,
    peer: SocketAddr,
    id: u64,
    events: Sender<Event>,
    stat: Arc<Mutex<Bytes>>,
)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut headers = UpgradeHeaders::default();
    let config = WebSocketConfig::default()
        .read_buffer_size(READ_BUFFER)
        .max_message_size(Some(MAX_PAYLOAD))
        .max_frame_size(Some(MAX_PAYLOAD));
    let upgrade = tokio_tungstenite::accept_hdr_async_with_config(
        stream,
        |request: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
            let offered = header(request, "sec-websocket-protocol");
            if let Some(first) = offered.as_deref().and_then(|list| list.split(',').next()) {
                if let Ok(value) = HeaderValue::from_str(first.trim()) {
                    response.headers_mut().insert("Sec-WebSocket-Protocol", value);
                }
            }
            headers.origin = header(request, "origin");
            headers.user_agent = header(request, "user-agent");
            headers.cache_control = header(request, "cache-control");
            Ok(response)
        },
        Some(config),
    );
    let Ok(Ok(ws)) = timeout(UPGRADE_TIMEOUT, upgrade).await else {
        return;
    };

    let outbox = Arc::new(Outbox::new());
    let info = ConnInfo {
        ip: peer.ip().to_canonical().to_string(),
        origin: headers.origin,
        user_agent: headers.user_agent,
        cache_control: headers.cache_control,
        outbox: outbox.clone(),
    };
    if events.send(Event::Open { id, info }).is_err() {
        return;
    }

    let (mut sink, mut stream) = ws.split();
    let reader = async {
        let mut decoder = Decoder::default();
        let mut budget = MessageBudget::new(Instant::now());
        let mut last_stat: Option<Instant> = None;
        loop {
            let data = match stream.next().await {
                Some(Ok(Message::Binary(data))) => data,
                Some(Ok(Message::Close(_))) | None => return Ended::Quiet,
                Some(Err(Error::Capacity(_))) => return Ended::ProtocolError(CLOSE_MESSAGE_TOO_BIG),
                Some(Err(Error::Protocol(_))) => return Ended::ProtocolError(CLOSE_PROTOCOL_ERROR),
                Some(Err(_)) => return Ended::Quiet,
                Some(Ok(_)) => continue,
            };
            if outbox.is_closing() {
                continue;
            }
            if data.len() > MAX_MESSAGE {
                outbox.close(CLOSE_MESSAGE_TOO_BIG, "Spam");
                continue;
            }
            match decoder.decode(&data) {
                Decoded::Msg(msg) => {
                    let now = Instant::now();
                    if !budget.allow(now) {
                        continue;
                    }
                    if matches!(msg, ClientMsg::Stat) {
                        if last_stat.is_none_or(|at| now - at >= STAT_SPACING) {
                            outbox.push(stat.lock().unwrap().clone());
                        }
                        last_stat = Some(now);
                    }
                    if events.send(Event::Message { id, msg }).is_err() {
                        return Ended::Quiet;
                    }
                }
                Decoded::Handled | Decoded::Ignored => {}
                Decoded::Close(code, reason) => outbox.close(code, &reason),
            }
        }
    };
    let writer = async {
        loop {
            outbox.notified().await;
            let Drained { frames, close } = outbox.drain();
            for frame in frames {
                if sink.feed(Message::Binary(frame)).await.is_err() {
                    return;
                }
            }
            if sink.flush().await.is_err() {
                return;
            }
            if let Some((code, reason)) = close {
                if sink.send(close_frame(code, reason)).await.is_err() {
                    return;
                }
            }
        }
    };
    let closing = async {
        outbox.closing().await;
        sleep(CLOSE_TIMEOUT).await;
    };

    let ended = tokio::select! {
        ended = reader => ended,
        _ = writer => Ended::Quiet,
        _ = closing => Ended::Expired,
    };
    match ended {
        Ended::ProtocolError(code) => {
            let farewell = sink.send(close_frame(code, String::new()));
            let _ = timeout(CLOSE_TIMEOUT, farewell).await;
        }
        Ended::Quiet => {
            let _ = timeout(CLOSE_TIMEOUT, sink.flush()).await;
        }
        Ended::Expired => {}
    }
    let _ = events.send(Event::Closed { id });
}
