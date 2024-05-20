use quinn::{Endpoint, ServerConfig};
use h3::{Config, Server };
use rustls::{Certificate, PrivateKey};
use rcgen::generate_simple_self_signed;
#[actix_rt::main]
async fn main() {
    // start a logger
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_writer(std::io::stderr)
        .init();

    

    let health_listen = std::env::var("HEALTH_LISTEN_URL")
        .expect("expected HEALTH_LISTEN_URL to be set")
        .to_socket_addr()
        .expect("expected HEALTH_LISTEN_URL to be a valid socket address")
        .next()
        .expect("expected HEALTH_LISTEN_URL to be a valid socket address");

    println!("Hello, world!");
}
