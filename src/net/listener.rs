use super::socket::{self, Event};
use super::tls::Acceptor;
use crate::core::logger;
use bytes::Bytes;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
#[cfg(feature = "tls")]
use tokio::time::timeout;

pub const ACCEPT_RETRY: Duration = Duration::from_millis(50);
#[cfg(feature = "tls")]
const TLS_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections that may be in the TLS or WebSocket handshake at once.
const MAX_HANDSHAKES: usize = 256;
/// Of those, how many may come from one address.
const MAX_HANDSHAKES_PER_IP: u32 = 8;
/// With `TCP_NOTSENT_LOWAT` the kernel holds at most this much unsent data per
/// connection, so a client that reads too slowly blocks the writer early and the
/// game stops building updates for it instead of letting them go stale in the
/// kernel's send buffer. Linux only.
#[cfg(any(target_os = "linux", target_os = "android"))]
const UNSENT_LIMIT: u32 = 16 * 1024;

/// A connection counted against the handshake limits until it is dropped.
pub struct Handshake {
    _permit: OwnedSemaphorePermit,
    ip: IpAddr,
    per_ip: Arc<Mutex<HashMap<IpAddr, u32>>>,
}

impl Drop for Handshake {
    fn drop(&mut self) {
        let mut per_ip = self.per_ip.lock().unwrap();
        if let Some(count) = per_ip.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                per_ip.remove(&self.ip);
            }
        }
    }
}

struct Limits {
    slots: Arc<Semaphore>,
    per_ip: Arc<Mutex<HashMap<IpAddr, u32>>>,
}

impl Limits {
    fn admit(&self, ip: IpAddr) -> Option<Handshake> {
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        let mut per_ip = self.per_ip.lock().unwrap();
        let count = per_ip.entry(ip).or_insert(0);
        if *count >= MAX_HANDSHAKES_PER_IP {
            return None;
        }
        *count += 1;
        Some(Handshake { _permit: permit, ip, per_ip: self.per_ip.clone() })
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn limit_unsent(stream: &TcpStream) {
    let _ = socket2::SockRef::from(stream).set_tcp_notsent_lowat(UNSENT_LIMIT);
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn limit_unsent(_stream: &TcpStream) {}

pub async fn accept_loop(
    listener: TcpListener,
    acceptor: Acceptor,
    events: Sender<Event>,
    stat: Arc<Mutex<Bytes>>,
) {
    let next_id = AtomicU64::new(1);
    let limits = Limits {
        slots: Arc::new(Semaphore::new(MAX_HANDSHAKES)),
        per_ip: Arc::new(Mutex::new(HashMap::new())),
    };
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                logger::error(&format!("accept failed: {e}"));
                tokio::time::sleep(ACCEPT_RETRY).await;
                continue;
            }
        };
        let Some(handshake) = limits.admit(peer.ip().to_canonical()) else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        limit_unsent(&stream);
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        let events = events.clone();
        let acceptor = acceptor.clone();
        let stat = stat.clone();
        tokio::spawn(handle_stream(stream, peer, id, events, stat, acceptor, handshake));
    }
}

async fn handle_stream(
    stream: TcpStream,
    peer: SocketAddr,
    id: u64,
    events: Sender<Event>,
    stat: Arc<Mutex<Bytes>>,
    acceptor: Acceptor,
    handshake: Handshake,
) {
    match acceptor {
        #[cfg(feature = "tls")]
        Acceptor::Tls(acceptor) => {
            if let Ok(Ok(stream)) = timeout(TLS_TIMEOUT, acceptor.accept(stream)).await {
                socket::serve(stream, peer, id, events, stat, handshake).await;
            }
        }
        Acceptor::Plain => socket::serve(stream, peer, id, events, stat, handshake).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(slots: usize) -> Limits {
        Limits { slots: Arc::new(Semaphore::new(slots)), per_ip: Arc::default() }
    }

    #[test]
    fn one_address_gets_a_bounded_share_of_handshakes() {
        let limits = limits(MAX_HANDSHAKES);
        let (a, b): (IpAddr, IpAddr) = ("10.0.0.1".parse().unwrap(), "10.0.0.2".parse().unwrap());
        let held: Vec<_> = (0..MAX_HANDSHAKES_PER_IP).map(|_| limits.admit(a).unwrap()).collect();
        assert!(limits.admit(a).is_none());
        assert!(limits.admit(b).is_some());
        drop(held);
        assert!(limits.admit(a).is_some());
        assert!(limits.per_ip.lock().unwrap().is_empty(), "counts are released on drop");
    }

    #[test]
    fn handshakes_are_bounded_in_total() {
        let limits = limits(2);
        let ip = |n: u8| IpAddr::from([10, 0, 0, n]);
        let first = limits.admit(ip(1)).unwrap();
        let _second = limits.admit(ip(2)).unwrap();
        assert!(limits.admit(ip(3)).is_none());
        drop(first);
        assert!(limits.admit(ip(3)).is_some());
    }
}
