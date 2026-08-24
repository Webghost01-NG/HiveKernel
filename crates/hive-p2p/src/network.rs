use crate::protocol::SwarmMessage;
use tokio::sync::broadcast::{self, Receiver, Sender};

/// Asynchronous P2P Swarm Gossip Router
#[derive(Clone)]
pub struct SwarmMeshRouter {
    sender: Sender<SwarmMessage>,
}

impl SwarmMeshRouter {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> Receiver<SwarmMessage> {
        self.sender.subscribe()
    }

    pub fn broadcast(&self, msg: SwarmMessage) -> Result<usize, String> {
        self.sender
            .send(msg)
            .map_err(|e| format!("Failed to broadcast message to swarm mesh: {}", e))
    }
}
