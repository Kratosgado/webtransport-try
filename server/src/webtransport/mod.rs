pub mod webtransport_opt;
pub mod get_key_and_cert_chain;
pub mod is_http3;
pub mod start;

pub const WEB_TRANSPORT_ALPN: &[&[u8]] = &[b"h3", b"h3-32", b"h3-31", b"h3-30", b"h3-29"];

pub const QUIC_ALPN: &[u8] = b"hq-29";

const MAX_UNIDIRECTIONAL_STREAM_SIZE: usize = 500_000;
