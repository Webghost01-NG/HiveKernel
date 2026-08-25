use crate::protocol::SwarmMessage;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub struct SwarmTcpClient;

impl SwarmTcpClient {
    pub async fn send_message(
        peer_addr: &str,
        msg: &SwarmMessage,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = TcpStream::connect(peer_addr).await?;
        let json = serde_json::to_string(msg)?;

        stream.write_all(json.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).await?;

        Ok(response.trim().to_string())
    }
}
