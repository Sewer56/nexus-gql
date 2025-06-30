use thiserror::Error;

/// Result type alias for Nexus operations
pub type Result<T> = std::result::Result<T, NexusError>;

/// Error types for Nexus Mods GraphQL client operations
#[derive(Error, Debug)]
pub enum NexusError {
    /// HTTP request errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// GraphQL errors from the API
    #[error("GraphQL error: {0}")]
    GraphQL(String),

    /// JSON parsing errors
    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    /// API returned no data
    #[error("API returned no data")]
    NoData,

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    Config(String),
}

impl NexusError {
    /// Create a new GraphQL error
    pub fn graphql<S: Into<String>>(message: S) -> Self {
        Self::GraphQL(message.into())
    }

    /// Create a new configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config(message.into())
    }
}
