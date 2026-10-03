use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use transcriber_api::{pcm, protocol::ServerMessage};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::args().skip_while(|x| x != "--addr").nth(1).unwrap_or_else(|| "ws://127.0.0.1:8765/v1/stream".into());
    let (mut ws, _) = connect_async(url).await?;
    ws.send(Message::Text(r#"{"type":"start","protocol":1,"audio":{"encoding":"pcm_s16le","sample_rate":16000,"channels":1}}"#.into())).await?;
    let first = ws.next().await.ok_or_else(|| anyhow::anyhow!("server closed before ready"))??;
    anyhow::ensure!(matches!(first, Message::Text(ref s) if matches!(serde_json::from_str::<ServerMessage>(s)?, ServerMessage::Ready { protocol: 1, sample_rate: 16000 })), "expected ready first");
    for chunk in pcm::fixture().chunks(3) { ws.send(Message::Binary(chunk.to_vec().into())).await?; tokio::time::sleep(std::time::Duration::from_millis(25)).await; }
    ws.send(Message::Text(r#"{"type":"stop"}"#.into())).await?;
    let mut completes = 0;
    while let Some(msg) = ws.next().await { if let Message::Text(s) = msg? { let parsed: ServerMessage = serde_json::from_str(&s)?; println!("{s}"); match parsed { ServerMessage::Complete { text } => { completes += 1; anyhow::ensure!(text == "hello world", "unexpected complete text: {text}"); }, ServerMessage::Error { .. } => anyhow::bail!("server returned an error: {s}"), _ => {} } } }
    anyhow::ensure!(completes == 1, "expected exactly one complete");
    Ok(())
}
