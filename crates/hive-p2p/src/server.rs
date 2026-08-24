use crate::protocol::SwarmMessage;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{error, info};

pub struct SwarmTcpNode {
    pub bind_addr: String,
    pub tx: broadcast::Sender<SwarmMessage>,
}

impl SwarmTcpNode {
    pub fn new(bind_addr: impl Into<String>, channel_capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(channel_capacity);
        Self {
            bind_addr: bind_addr.into(),
            tx,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("🐝 HiveKernel P2P Node listening on {}", self.bind_addr);

        let tx = self.tx.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(socket, tx).await {
                                error!("Connection error from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_connection(
        mut socket: TcpStream,
        tx: broadcast::Sender<SwarmMessage>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, mut writer) = socket.split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        while buf_reader.read_line(&mut line).await? > 0 {
            if let Ok(msg) = serde_json::from_str::<SwarmMessage>(&line) {
                let _ = tx.send(msg);
                writer.write_all(b"{\"status\":\"ACK\"}\n").await?;
            } else {
                writer.write_all(b"{\"error\":\"MALFORMED_FRAME\"}\n").await?;
            }
            line.clear();
        }

        Ok(())
    }
}
