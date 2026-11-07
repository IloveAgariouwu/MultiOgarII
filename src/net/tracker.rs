use crate::core::logger;
use crate::world::VERSION;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub async fn ping_tracker(body: String) {
    let request = format!(
        "POST /master HTTP/1.0\r\nHost: ogar.mivabe.nl\r\nUser-Agent: MultiOgarII{VERSION}\r\n\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    match TcpStream::connect(("ogar.mivabe.nl", 80)).await {
        Ok(mut stream) => {
            let _ = stream.write_all(request.as_bytes()).await;
        }
        Err(e) => logger::warn(&format!("Tracker: {e}")),
    }
}
