use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::{net::TcpListener, sync::Semaphore, time::{timeout, Duration}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use transcriber_api::protocol::ServerMessage;

async fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let sessions = Arc::new(Semaphore::new(1));
    tokio::spawn(async move { loop { let (stream, _) = listener.accept().await.unwrap(); tokio::spawn(transcriber_api::server::connection(stream, sessions.clone())); } });
    format!("ws://{addr}/v1/stream")
}
async fn connect(url: &str) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    connect_async(url).await.unwrap().0
}
async fn start(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) {
    ws.send(Message::Text(r#"{"type":"start","protocol":1,"audio":{"encoding":"pcm_s16le","sample_rate":16000,"channels":1}}"#.into())).await.unwrap();
    let msg = ws.next().await.unwrap().unwrap();
    let text = match msg { Message::Text(s) => s, _ => panic!() };
    assert!(matches!(serde_json::from_str::<ServerMessage>(&text).unwrap(), ServerMessage::Ready { .. }));
}
#[tokio::test]
async fn stop_has_one_complete() {
    let url = server().await; let mut ws = connect(&url).await; start(&mut ws).await;
    ws.send(Message::Binary(vec![0, 0].into())).await.unwrap(); ws.send(Message::Text(r#"{"type":"stop"}"#.into())).await.unwrap();
    let mut complete = Vec::new();
    timeout(Duration::from_secs(1), async {
        while let Some(Ok(Message::Text(s))) = ws.next().await {
            if let ServerMessage::Complete { text } = serde_json::from_str(&s).unwrap() { complete.push(text); }
        }
    }).await.unwrap();
    assert_eq!(complete, ["hello world"]);
}
#[tokio::test]
async fn cancel_and_disconnect_do_not_complete() {
    let url = server().await; let mut ws = connect(&url).await; start(&mut ws).await;
    ws.send(Message::Text(r#"{"type":"cancel"}"#.into())).await.unwrap();
    ws.close(None).await.unwrap();
    let mut complete = false;
    timeout(Duration::from_secs(1), async {
        while let Some(Ok(Message::Text(s))) = ws.next().await {
            complete |= matches!(serde_json::from_str::<ServerMessage>(&s).unwrap(), ServerMessage::Complete { .. });
        }
    }).await.unwrap();
    assert!(!complete);
    let mut ws = connect(&url).await; start(&mut ws).await;
    ws.close(None).await.unwrap();
    timeout(Duration::from_secs(1), async { while ws.next().await.is_some() {} }).await.unwrap();
    let mut ws = connect(&url).await; start(&mut ws).await; ws.send(Message::Text(r#"{"type":"stop"}"#.into())).await.unwrap();
    assert!(timeout(Duration::from_secs(1), async { while let Some(Ok(Message::Text(s))) = ws.next().await { if matches!(serde_json::from_str::<ServerMessage>(&s).unwrap(), ServerMessage::Complete { .. }) { return true; } } false }).await.unwrap());
}
#[tokio::test]
async fn busy_and_overrun_are_structured_errors() {
    let url = server().await; let mut first = connect(&url).await; start(&mut first).await;
    let mut second = connect(&url).await; second.send(Message::Text(r#"{"type":"start","protocol":1,"audio":{"encoding":"pcm_s16le","sample_rate":16000,"channels":1}}"#.into())).await.unwrap();
    let text = match second.next().await.unwrap().unwrap() { Message::Text(s) => s, _ => panic!() };
    assert!(matches!(serde_json::from_str::<ServerMessage>(&text).unwrap(), ServerMessage::Error { code, .. } if code == "busy"));
    first.send(Message::Binary(vec![0, 0].into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1)).await;
    for _ in 0..3 { first.send(Message::Binary(vec![0, 0].into())).await.unwrap(); }
    let got = timeout(Duration::from_secs(1), async { while let Some(Ok(Message::Text(s))) = first.next().await { if let Ok(ServerMessage::Error { code, .. }) = serde_json::from_str(&s) { return code; } } "".into() }).await.unwrap();
    assert_eq!(got, "audio_overrun");
}
