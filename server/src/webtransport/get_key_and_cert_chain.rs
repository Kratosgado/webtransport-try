use anyhow::Context;
use rustls::{Certificate, PrivateKey};

use super::webtransport_opt::Certs;

pub fn get_key_and_cert_chain(certs: Certs) -> anyhow::Result<(PrivateKey, Vec<Certificate>)> {
    let key_path = certs.key;
    let cert_path = certs.cert;
    let key = std::fs::read(&key_path).context("failed to read private key")?;
    let key = if key_path.extension().map_or(false, |ext| ext == "der") {
        PrivateKey(key)
    } else {
        let pkcs8 = rustls_pemfile::pkcs8_private_keys(&mut &*key)
            .context("malformed PKCS #8 private key")?;
        match pkcs8.into_iter().next() {
            Some(x) => PrivateKey(x),
            None => {
                let rsa = rustls_pemfile::rsa_private_keys(&mut &*key)
                    .context("malformed RSA private key")?;
                match rsa.into_iter().next() {
                    Some(x) => PrivateKey(x),
                    None => return Err(anyhow::anyhow!("no private key found")),
                }
            }
        }
    };
    let certs = std::fs::read(&cert_path).context("failed to read certificate chain")?;
    let certs = if cert_path.extension().map_or(false, |ext| ext == "der") {
        vec![Certificate(certs)]
    } else {
        rustls_pemfile::certs(&mut &*certs)
            .context("failed to read PEM-encoded certificate chain")?
            .into_iter()
            .map(Certificate)
            .collect()
    };
    Ok((key, certs))
}
