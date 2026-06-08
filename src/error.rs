use thiserror::Error;

#[derive(Error, Debug)]
pub enum IshError {
    #[error("Configuration Error: {0}")]
    ConfigError(String),

    #[error("Parse Error: {0}")]
    ParseError(String),

    #[error("Execution Error: {0}")]
    ExecutionError(String),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unknown error occurred")]
    Unknown,
}
