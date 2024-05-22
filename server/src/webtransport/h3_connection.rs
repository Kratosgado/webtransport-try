
use anyhow::Result;
use bytes::Bytes;
use sec_http3::{error::ErrorLevel, sec_http3_quinn, server::Connection};
use tracing::{error, info};



pub async fn handle_h3_connection(
    mut conn: Connection<sec_http3_quinn::Connection, Bytes>,
    nc: async_nats::Client,
) -> Result<()> {
    // 3. TODO: Conditionally, if the client indicated that this is a webtransport session, we should accept it here, else use regular h3.
    // if this is a webtransport session, then h3 needs to stop handing the datagrams, bidirectional streams, and unidirectional streams and give them to the webtransport session.

    loop {
        match conn.accept().await {
            Ok(Some((req, stream) )) => {
                info!("new request");
            },
            Ok(None) => {
                info!("no more requests");
                break;
            },
            Err(err ) => {
                error!("error on accept: {}", err);
                match err.get_error_level() {
                    ErrorLevel::ConnectionError => break,
                    ErrorLevel::StreamError => continue,
                }
            }
        }
    }
    Ok(())
}