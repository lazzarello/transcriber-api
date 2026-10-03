use std::{env, net::SocketAddr};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8765".into()).parse()?;
    println!("listening on {addr}");
    transcriber_api::server::serve(addr).await
}
