use crate::{fake_transcriber::{Event, Fake}, pcm::Decoder, protocol::*};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::{IpAddr, SocketAddr};
use tokio::{net::{TcpListener, TcpStream}, sync::{mpsc, oneshot, Semaphore}};
use tokio_tungstenite::{accept_hdr_async_with_config, tungstenite::{handshake::server::{Request, Response}, http::StatusCode, Message, protocol::WebSocketConfig}, WebSocketStream};

pub fn validate_bind(addr: SocketAddr) -> bool {
    match addr.ip() { IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local() || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])), IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() }
}

pub async fn serve(addr: SocketAddr) -> Result<()> {
    if !validate_bind(addr) { anyhow::bail!("refusing non-private bind; use loopback or Tailscale/private address") }
    let listener = TcpListener::bind(addr).await?;
    let sessions = std::sync::Arc::new(Semaphore::new(1));
    loop {
        let (stream, _) = listener.accept().await?;
        let sessions = sessions.clone();
        tokio::spawn(async move { let _ = connection(stream, sessions).await; });
    }
}

pub async fn connection(stream: TcpStream, sessions: std::sync::Arc<Semaphore>) -> Result<()> {
    let ws = accept_hdr_async_with_config(stream, |request: &Request, response: Response| {
        if request.uri().path() == "/v1/stream" { Ok(response) } else {
            Err(Response::builder().status(StatusCode::NOT_FOUND).body(Some("not found".to_string())).unwrap())
        }
    }, Some(WebSocketConfig { max_message_size: Some(64 * 1024), max_frame_size: Some(64 * 1024), ..Default::default() })).await?;
    let (mut sink, mut input) = ws.split();
    let first = match input.next().await {
        Some(Ok(Message::Text(s))) => s,
        Some(Ok(_)) | Some(Err(_)) => { send_error(&mut sink, "bad_request", "first message must be start").await?; return Ok(()) },
        None => return Ok(()),
    };
    let start: ClientMessage = match serde_json::from_str(&first) { Ok(x) => x, Err(_) => { send_error(&mut sink, "bad_request", "first message must be start").await?; return Ok(()) } };
    let ClientMessage::Start { protocol, audio } = start else { send_error(&mut sink, "bad_request", "first message must be start").await?; return Ok(()) };
    if let Err((c, m)) = validate_start(protocol, &audio) { send_error(&mut sink, c, m).await?; return Ok(()) }
    let permit = match sessions.try_acquire() { Ok(p) => p, Err(_) => { send_error(&mut sink, "busy", "another session is active").await?; return Ok(()) } };
    sink.send(Message::Text(serde_json::to_string(&ServerMessage::Ready { protocol: 1, sample_rate: 16000 })?.into())).await?;
    let (audio_tx, audio_rx) = mpsc::channel(2);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let (cancel_sender, cancel_rx) = oneshot::channel();
    let mut cancel_tx = Some(cancel_sender);
    tokio::spawn(Fake { audio: audio_rx, events: event_tx, cancel: cancel_rx }.run());
    let mut decoder = Decoder::new();
    let mut audio_tx = Some(audio_tx);
    let mut snapshot = Snapshot::default();
    loop {
        tokio::select! {
            message = input.next() => {
                let Some(message) = message else { let _ = cancel_tx.take().map(|tx| tx.send(())); break };
                match message? {
                    Message::Binary(bytes) => {
                        if bytes.len() > 64 * 1024 { send_error(&mut sink, "bad_request", "audio message too large").await?; let _ = cancel_tx.take().map(|tx| tx.send(())); break }
                        let samples = decoder.push(&bytes);
                        if let Some(tx) = &audio_tx { if tx.try_send(samples).is_err() { send_error(&mut sink, "audio_overrun", "audio queue is full; retry").await?; let _ = cancel_tx.take().map(|tx| tx.send(())); break } }
                    }
                    Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(ClientMessage::Stop) => { if decoder.finish().is_err() { send_error(&mut sink, "bad_request", "odd trailing PCM byte").await?; let _ = cancel_tx.take().map(|tx| tx.send(())); break } audio_tx = None; }
                        Ok(ClientMessage::Cancel) => { let _ = cancel_tx.take().map(|tx| tx.send(())); break; }
                        Ok(ClientMessage::Ping) => { sink.send(Message::Text(serde_json::to_string(&ServerMessage::Pong)?.into())).await?; }
                        _ => { send_error(&mut sink, "bad_request", "unexpected message").await?; }
                    },
                    Message::Close(_) => { let _ = cancel_tx.take().map(|tx| tx.send(())); break }
                    _ => {}
                }
            }
            event = event_rx.recv() => {
                match event {
                    Some(Event::Partial(t)) => { snapshot.append(&t); snapshot.changed(); send_snapshot(&mut sink, false, &snapshot).await?; }
                    Some(Event::Final(t)) => { snapshot.final_delta(&t); snapshot.changed(); send_snapshot(&mut sink, true, &snapshot).await?; }
                    Some(Event::Replace { backspace, text }) => { snapshot.replace(backspace, &text); snapshot.changed(); send_snapshot(&mut sink, false, &snapshot).await?; }
                    Some(Event::Ended) => { sink.send(Message::Text(serde_json::to_string(&ServerMessage::Complete { text: snapshot.text() })?.into())).await?; break }
                    Some(Event::Error(e)) => { send_error(&mut sink, "transcription_failed", &e).await?; break }
                    None => break,
                }
            }
        }
    }
    sink.close().await?;
    drop(permit);
    Ok(())
}

async fn send_error<S: SinkExt<Message> + Unpin>(sink: &mut S, code: &str, message: &str) -> Result<()> where S::Error: std::error::Error + Send + Sync + 'static { sink.send(Message::Text(serde_json::to_string(&ServerMessage::Error { code: code.into(), message: message.into() })?.into())).await?; Ok(()) }
async fn send_snapshot<S: SinkExt<Message> + Unpin>(sink: &mut S, final_: bool, s: &Snapshot) -> Result<()> where S::Error: std::error::Error + Send + Sync + 'static { let m = if final_ { ServerMessage::Final { text: s.text(), revision: s.revision } } else { ServerMessage::Partial { text: s.text(), revision: s.revision } }; sink.send(Message::Text(serde_json::to_string(&m)?.into())).await?; Ok(()) }

#[allow(dead_code)]
fn _ws_type(_: WebSocketStream<TcpStream>) {}
