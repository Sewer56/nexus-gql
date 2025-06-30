//! Custom types for GraphQL scalar handling

use bytesize::ByteSize;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::str::FromStr;

/// A custom ByteSize type that deserializes from string.
///
/// The Nexus API returns large integer (BigInt) fields as strings (e.g., `"905633855"`) rather than numbers,
/// but we want to work with them as proper ByteSize values for arithmetic operations and human-readable formatting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSizeString(pub ByteSize);

impl<'de> Deserialize<'de> for ByteSizeString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let value = u64::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(ByteSizeString(ByteSize::b(value)))
    }
}

impl fmt::Display for ByteSizeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_u64())
    }
}

impl ByteSizeString {
    /// Get the inner u64 value
    pub fn value(&self) -> u64 {
        self.0.as_u64()
    }

    /// Get the value in bytes (useful for file sizes)
    pub fn bytes(&self) -> u64 {
        self.0.as_u64()
    }

    /// Get the ByteSize object
    pub fn byte_size(&self) -> ByteSize {
        self.0
    }

    /// Format as human-readable size (KB, MB, GB, etc.)
    pub fn format_bytes(&self) -> String {
        self.0.to_string()
    }

    /// Create from a u64 value
    pub fn from_u64(value: u64) -> Self {
        ByteSizeString(ByteSize::b(value))
    }

    /// Create from a ByteSize value
    pub fn from_byte_size(size: ByteSize) -> Self {
        ByteSizeString(size)
    }
}

/// Download link information from the legacy REST API
///
/// This represents a download mirror/link returned by the download_link.json endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DownloadLink {
    /// Short name identifier for the download mirror (e.g., "Nexus CDN", "Premium")
    #[serde(rename = "short_name")]
    pub short_name: String,

    /// Direct download URL
    #[serde(rename = "URI")]
    pub uri: String,
}

impl DownloadLink {
    /// Create a new download link
    pub fn new(short_name: String, uri: String) -> Self {
        Self { short_name, uri }
    }

    /// Get the mirror name
    pub fn mirror_name(&self) -> &str {
        &self.short_name
    }

    /// Get the download URL
    pub fn url(&self) -> &str {
        &self.uri
    }
}

impl fmt::Display for DownloadLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.short_name, self.uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytesize_string_deserialization() {
        // Test deserializing from JSON string
        let json = r#""905633855""#;
        let size: ByteSizeString = serde_json::from_str(json).unwrap();

        assert_eq!(size.to_string(), "905633855");
        assert_eq!(size.value(), 905633855);
        assert_eq!(size.bytes(), 905633855);
        assert_eq!(size.format_bytes(), "863.7 MiB");
    }

    #[test]
    fn bytesize_string_comparison() {
        let a = ByteSizeString::from_u64(100);
        let b = ByteSizeString::from_u64(200);

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, ByteSizeString::from_u64(100));
    }

    #[test]
    fn bytesize_formatting() {
        let size = ByteSizeString::from_u64(1024);
        assert_eq!(size.format_bytes(), "1.0 KiB");

        let size = ByteSizeString::from_u64(1536);
        assert_eq!(size.format_bytes(), "1.5 KiB");

        let size = ByteSizeString::from_u64(1048576);
        assert_eq!(size.format_bytes(), "1.0 MiB");

        let size = ByteSizeString::from_u64(905633855);
        assert_eq!(size.format_bytes(), "863.7 MiB");
    }
}
