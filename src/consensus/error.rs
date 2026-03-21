use std::error::Error;
use std::fmt;

/// Consensus-level error type used across mining / DAG / sync logic.
#[derive(Debug)]
pub enum ConsensusError {
    /// A general consensus validation or state error.
    InvalidState(String),
    /// Underlying IO or storage error.
    Io(String),
    /// Any other error.
    Other(String),
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusError::InvalidState(s) => write!(f, "Consensus state error: {}", s),
            ConsensusError::Io(s) => write!(f, "IO error: {}", s),
            ConsensusError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl Error for ConsensusError {}

impl From<String> for ConsensusError {
    fn from(s: String) -> Self {
        ConsensusError::Other(s)
    }
}

impl From<&str> for ConsensusError {
    fn from(s: &str) -> Self {
        ConsensusError::Other(s.to_string())
    }
}
