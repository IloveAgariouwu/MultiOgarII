use super::listener::ACCEPT_RETRY;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

const STATS_READ_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn stats_loop(listener: TcpListener, stats: Arc<Mutex<String>>) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            tokio::time::sleep(ACCEPT_RETRY).await;
            continue;
        };
        let stats = stats.clone();
        tokio::spawn(async move {
            let mut request = [0u8; 1024];
            let _ = timeout(STATS_READ_TIMEOUT, stream.read(&mut request)).await;
            let body = stats.lock().unwrap().clone();
            let response = format!(
                "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}
