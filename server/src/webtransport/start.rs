use std::{sync::Arc, time::Duration};

use quinn::VarInt;
use sec_http3::sec_http3_quinn;
use tracing::{info, trace_span};

use crate::webtransport::{is_http3::is_http3, QUIC_ALPN, WEB_TRANSPORT_ALPN};

use super::webtransport_opt::WebTransportOpt;

pub async fn start(opt: WebTransportOpt) -> Result<(), Box<dyn std::error::Error>> {
    info!("WebTransportOpt: {opt:#?}");

    let (key, certs) = crate::webtransport::get_key_and_cert_chain(opt.certs)?;

    let mut tls_config = rustls::ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    tls_config.max_early_data_size = u32::MAX;
    let mut alpn = vec![];
    for proto in WEB_TRANSPORT_ALPN {
        alpn.push(proto.to_vec());
    }
    alpn.push(QUIC_ALPN.to_vec());

    tls_config.alpn_protocols = alpn;

    // 1. create quinn server endpoint and bind UDP socket
    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(tls_config));
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(2)));
    transport_config.max_idle_timeout(Some(VarInt::from_u32(10_000).into()));
    server_config.transport = Arc::new(transport_config);
    let endpoint = quinn::Endpoint::server(config, opt.listen)?;

    info!("Listening on: {}", opt.listen);

    let nc = async_nats::connect(std::env::var("NATS_URL").expect("NATS_URL env var must be defined")).await.unwrap();

    // 2. accept new quic connections and spawn a new task to handle them
    while let Some(new_conn ) = endpoint.accept().await {
        trace_span!("New connection being attempted");
        let nc = nc.clone();

        tokio::spawn(async move {
            match new_conn.await {
                Ok(conn ) => {
                    if is_http3(&conn){
                        info!("new http3 established");
                        let h3_conn = sec_http3::server::builder()
                            .enable_webtransport(true)
                            .enable_connect(true)
                            .enable_datagram(true)
                            .max_webtransport_sessions(1)
                            .send_grease(true)
                            .build(sec_http3_quinn::Connection::new(conn))
                            .await
                            .unwrap();
                        let nc = nc.clone();
                        // if let Err(err ) = handle
                    }
                    else {
                        info!("new quic established");
                        let nc = nc.clone();
                        let (send, recv) = conn.open_bi().await.unwrap();
                        let (mut send, mut recv) = (send.clone(), recv.clone());
                        let nc = nc.clone();
                        tokio::spawn(async move {
                            let mut buf = vec![0; 1024];
                            loop {
                                let n = recv.read(&mut buf).await.unwrap();
                                if n == 0 {
                                    break;
                                }
                                let msg = std::str::from_utf8(&buf[..n]).unwrap();
                                println!("Received: {}", msg);
                                let _ = nc.publish("webtransport", msg).await;
                            }
                        });
                        tokio::spawn(async move {
                            loop {
                                let msg = nc.subscribe("webtransport").await.unwrap();
                                send.write_all(msg.as_bytes()).await.unwrap();
                            }
                        });
                    }
                }
            }
        })
    }
}