use thiserror::Error;

#[derive(Error, Debug)]
pub enum IshIOError {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    
    #[error("Invalid hex color: {0}")]
    InvalidHexColor(String),
    
    #[error("Stream error: {0}")]
    StreamError(String),
    
    #[error("Terminal error: {0}")]
    TerminalError(String),
}
