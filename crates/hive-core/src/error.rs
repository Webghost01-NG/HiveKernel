use thiserror::Error;

#[derive(Error, Debug)]
pub enum HiveError {
    #[error("Cryptographic signature verification failed: {0}")]
    SignatureError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Task execution failed: {0}")]
    TaskExecutionError(String),

    #[error("Escrow violation: {0}")]
    EscrowError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Invalid state transition: {0}")]
    StateError(String),

    #[error("Timeout or deadline expired: {0}")]
    TimeoutError(String),
}

pub type Result<T> = std::result::Result<T, HiveError>;
