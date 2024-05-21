use quinn::crypto::rustls::HandshakeData;

use super::WEB_TRANSPORT_ALPN;

pub fn is_http3(conn: &quinn::Connection) -> bool {
    if let Some(data) = conn.handshake_data() {
        if let Some(d) = data.downcast_ref::<HandshakeData>() {
            if let Some(alpn) = &d.protocol {
                return WEB_TRANSPORT_ALPN.contains(&alpn.as_slice());
            }
        }
    };
    false
}
