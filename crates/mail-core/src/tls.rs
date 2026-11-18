use std::sync::{Arc, OnceLock};

use rustls_platform_verifier::BuilderVerifierExt;
use tokio_rustls::rustls::{self, pki_types::ServerName};
use tokio_rustls::TlsConnector;

use crate::{Error, Result};

/// OS sertifikatlari ombori bilan tekshiradigan TLS connector (korporativ CA'lar ham ishlaydi).
pub fn connector() -> Result<TlsConnector> {
    static CONFIG: OnceLock<std::result::Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    let cfg = CONFIG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_platform_verifier()
            .map(|b| Arc::new(b.with_no_client_auth()))
            .map_err(|e| e.to_string())
    });
    match cfg {
        Ok(c) => Ok(TlsConnector::from(c.clone())),
        Err(e) => Err(Error::Tls(e.clone())),
    }
}

pub fn server_name(host: &str) -> Result<ServerName<'static>> {
    ServerName::try_from(host.to_string()).map_err(|_| Error::Tls(format!("invalid host name: {host}")))
}
