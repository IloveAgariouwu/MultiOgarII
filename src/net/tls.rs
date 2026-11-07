use crate::core::logger;
#[cfg(feature = "tls")]
use std::fs::File;
#[cfg(feature = "tls")]
use std::io::BufReader;
#[cfg(feature = "tls")]
use std::path::Path;
#[cfg(feature = "tls")]
use std::sync::Arc;
#[cfg(feature = "tls")]
use tokio_rustls::rustls::ServerConfig;
#[cfg(feature = "tls")]
use tokio_rustls::TlsAcceptor;

#[derive(Clone)]
pub enum Acceptor {
    Plain,
    #[cfg(feature = "tls")]
    Tls(TlsAcceptor),
}

#[cfg(feature = "tls")]
pub fn load_tls() -> Acceptor {
    let (key, cert) = (Path::new("keys/key.pem"), Path::new("keys/cert.pem"));
    if !(key.exists() && cert.exists()) {
        logger::info("Using HTTP");
        return Acceptor::Plain;
    }
    match load_acceptor(key, cert) {
        Ok(acceptor) => {
            logger::info("Using HTTPS, use the wss:// protocol prefix when connecting");
            Acceptor::Tls(acceptor)
        }
        Err(e) => {
            logger::error(&format!("Failed to load TLS keys, using HTTP: {e}"));
            Acceptor::Plain
        }
    }
}

#[cfg(not(feature = "tls"))]
pub fn load_tls() -> Acceptor {
    logger::info("Using HTTP");
    Acceptor::Plain
}

#[cfg(feature = "tls")]
fn load_acceptor(key_path: &Path, cert_path: &Path) -> Result<TlsAcceptor, String> {
    let certs = rustls_pemfile::certs(&mut BufReader::new(
        File::open(cert_path).map_err(|e| format!("{}: {e}", cert_path.display()))?,
    ))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("{}: {e}", cert_path.display()))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(
        File::open(key_path).map_err(|e| format!("{}: {e}", key_path.display()))?,
    ))
    .map_err(|e| format!("{}: {e}", key_path.display()))?
    .ok_or_else(|| format!("{}: no private key found", key_path.display()))?;

    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| e.to_string())?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}
