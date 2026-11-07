use super::socket::{self, Event};
use super::tls::Acceptor;
use crate::core::logger;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use bytes::Bytes;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
#[cfg(feature = "tls")]
use tokio::time::timeout;

pub const ACCEPT_RETRY: Duration = Duration::from_millis(50);
#[cfg(feature = "tls")]
const TLS_TIMEOUT: Duration = Duration::from_secs(120);

pub async fn accept_loop(
    listener: TcpListener,
    acceptor: Acceptor,
    events: Sender<Event>,
    stat: Arc<Mutex<Bytes>>,
) {
    let next_id = AtomicU64::new(1);
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                logger::error(&format!("accept failed: {e}"));
                tokio::time::sleep(ACCEPT_RETRY).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        let events = events.clone();
        let acceptor = acceptor.clone();
        let stat = stat.clone();
        tokio::spawn(handle_stream(stream, peer, id, events, stat, acceptor));
    }
}

async fn handle_stream(
    stream: TcpStream,
    peer: SocketAddr,
    id: u64,
    events: Sender<Event>,
    stat: Arc<Mutex<Bytes>>,
    acceptor: Acceptor,
) {
    match acceptor {
        #[cfg(feature = "tls")]
        Acceptor::Tls(acceptor) => {
            if let Ok(Ok(stream)) = timeout(TLS_TIMEOUT, acceptor.accept(stream)).await {
                socket::serve(stream, peer, id, events, stat).await;
            }
        }
        Acceptor::Plain => socket::serve(stream, peer, id, events, stat).await,
    }
}
