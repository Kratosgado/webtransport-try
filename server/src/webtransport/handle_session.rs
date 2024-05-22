use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::anyhow;
use bytes::Bytes;
use futures::{AsyncRead, AsyncWrite, StreamExt};
use sec_http3::{
    quic::{self, RecvDatagramExt, SendDatagramExt, SendStreamUnframed},
    webtransport::{server::WebTransportSession, stream},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
};
use tracing::info;

#[tracing::instrument(level = "trace", skip(session))]
pub async fn handle_session<C>(
    session: WebTransportSession<C, Bytes>,
    username: &str,
    lobby_id: &str,
    nc: async_nats::client::Client,
) -> anyhow::Result<()>
where
    // use trait bounds to ensure we only happen to use implementation that are only for the quinn backend.
    C: 'static
        + Send
        + sec_http3::quic::Connection<Bytes>
        + RecvDatagramExt<Buf = Bytes>
        + SendDatagramExt<Bytes>,
    <C::SendStream as sec_http3::quic::SendStream<Bytes>>::Error:
        'static + std::error::Error + Send + Sync + Into<std::io::Error>,
    <C::RecvStream as sec_http3::quic::RecvStream>::Error:
        'static + std::error::Error + Send + Sync + Into<std::io::Error>,
    stream::BidiStream<C::BidiStream, Bytes>:
        quic::BidiStream<Bytes> + Unpin + AsyncWrite + AsyncRead,
    <stream::BidiStream<C::BidiStream, Bytes> as quic::BidiStream<Bytes>>::SendStream:
        Unpin + AsyncWrite + Send + Sync,
    <stream::BidiStream<C::BidiStream, Bytes> as quic::BidiStream<Bytes>>::RecvStream:
        Unpin + AsyncRead + Send + Sync,
    C::SendStream: Send + Sync + Unpin,
    C::RecvStream: Send + Unpin,
    C::BidiStream: Send + Unpin,
    stream::SendStream<C::SendStream, Bytes>: AsyncWrite,
    C::BidiStream: SendStreamUnframed<Bytes>,
    C::SendStream: SendStreamUnframed<Bytes> + Send,
    <C as sec_http3::quic::Connection<Bytes>>::OpenStreams: Send,
    <C as sec_http3::quic::Connection<Bytes>>::BidiStream: Sync,
{
    let session_id = session.session_id();
    let session = Arc::new(RwLock::new(session));
    let should_run = Arc::new(AtomicBool::new(true));

    let subject = format!("room.{}.*", lobby_id).replace(" ", "_");
    let specific_subject = format!("room.{}.{}", lobby_id, username).replace(" ", "_");
    let mut sub = match nc
        .queue_subscribe(subject.clone(), specific_subject.clone())
        .await
    {
        Ok(sub) => {
            info!("Subscribed to NATS subject: {}", subject);
            sub
        }
        Err(err) => {
            tracing::error!("Error subscribing to NATS subject {}: {}", subject, err);
            return Err(anyhow!(err));
        }
    };

    let specific_subject_clone = specific_subject.clone();

    let nats_task = {
        let session = session.clone();
        let should_run = should_run.clone();
        tokio::spawn(async move {
            while let Some(msg) = sub.next().await {
                if !should_run.load(Ordering::SeqCst) {
                    break;
                }
                if msg.subject == specific_subject_clone {
                    continue;
                }
                let session = session.read().await;
                if msg.payload.len() > 400 {
                    let stream = session.open_uni(session_id).await;
                    tokio::spawn(async move {
                        match stream {
                            Ok(mut uni_stream) => {
                                if let Err(e) = uni_stream.write_all(&msg.payload).await {
                                    tracing::error!(
                                        "Error writing to unidirectional stream: {}",
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error opening unidirectional stream: {}", e);
                            }
                        }
                    });
                } else if let Err(e) = session.send_datagram(msg.payload) {
                    tracing::error!("Error sending datagram: {}", e);
                }
            }
        })
    };

    let quic_task = {
        let session = session.clone();
        let nc = nc.clone();
        let specific_subject = specific_subject.clone();
        tokio::spawn(async move {
            let session = session.read().await;
            while let Ok(uni_stream) = session.accept_uni().await {
                if let Some((_id, mut uni_stream)) = uni_stream {
                    let nc = nc.clone();
                    let specific_subject = specific_subject.clone();
                    tokio::spawn(async move {
                        let mut buf = Vec::new();
                        // let specific_subject = specific_subject.clone();
                        if let Err(e) = uni_stream.read_to_end(&mut buf).await {
                            tracing::error!("Error reading from unidirectional stream: {}", e);
                        }
                        if let Err(e) = nc.publish(specific_subject.clone(), buf.into()).await {
                            tracing::error!("Error publishing to NATS{}: {}", specific_subject, e);
                        }
                    });
                }
            }
        })
    };

    let _datagrams_task = {
        tokio::spawn(async move {
            let session = session.read().await;
            while let Ok(datagram) = session.accept_datagram().await {
                if let Some((_id, buf)) = datagram {
                    let nc = nc.clone();
                    if let Err(e) = nc.publish(specific_subject.clone(), buf).await {
                        tracing::error!("Error publishing to NATS{}: {}", specific_subject, e);
                    }
                }
            }
        })
    };

    quic_task.await?;
    should_run.store(false, Ordering::SeqCst);
    nats_task.abort();
    info!("finished handling session");
    Ok(())
}
