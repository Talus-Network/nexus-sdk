use {ark_serialize::SerializationError, thiserror::Error};

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("synthesis error")]
    Synthesis,
    #[error("invalid contribution: {0}")]
    InvalidContribution(&'static str),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Ser(#[from] SerializationError),
    #[error("format error: {0}")]
    Format(&'static str),
}
