//! Custom types for GraphQL scalar handling

use bytesize::ByteSize;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
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

/// A wrapper around PathBuf specifically for archive file paths
///
/// This makes it clear that the path refers to a file within an archive,
/// and provides convenient methods for working with archive paths.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchiveFilePath(PathBuf);

impl ArchiveFilePath {
    /// Create a new archive file path
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self(path.into())
    }

    /// Get the file name from the path
    pub fn file_name(&self) -> Option<&str> {
        self.0.file_name()?.to_str()
    }

    /// Get the file extension
    pub fn extension(&self) -> Option<&str> {
        self.0.extension()?.to_str()
    }

    /// Get the directory path
    pub fn parent(&self) -> Option<&Path> {
        self.0.parent()
    }

    /// Check if this is a directory (ends with slash)
    pub fn is_directory(&self) -> bool {
        let path_str = self.0.to_string_lossy();
        path_str.ends_with('/') || path_str.ends_with('\\')
    }

    /// Get the path as a string
    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }

    /// Convert to PathBuf
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl Deref for ArchiveFilePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ArchiveFilePath {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Display for ArchiveFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl<'de> Deserialize<'de> for ArchiveFilePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(ArchiveFilePath::new(s))
    }
}

impl From<String> for ArchiveFilePath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for ArchiveFilePath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<PathBuf> for ArchiveFilePath {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<&Path> for ArchiveFilePath {
    fn from(path: &Path) -> Self {
        Self(path.to_path_buf())
    }
}

/// Information about a file contained within a mod archive
///
/// This represents a file entry returned by the files.json endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchiveFile {
    /// Full path of the file within the archive
    pub path: ArchiveFilePath,

    /// Size of the file in bytes
    pub size: u64,

    /// File modification timestamp (optional)
    #[serde(rename = "modified_time")]
    pub modified_time: Option<u64>,
}

impl ArchiveFile {
    /// Create a new archive file entry
    pub fn new<P: Into<ArchiveFilePath>>(path: P, size: u64, modified_time: Option<u64>) -> Self {
        Self {
            path: path.into(),
            size,
            modified_time,
        }
    }

    /// Get the file name from the path
    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name()
    }

    /// Get the file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension()
    }

    /// Get the directory path
    pub fn directory(&self) -> Option<&Path> {
        self.path.parent()
    }

    /// Format the file size as human-readable
    pub fn format_size(&self) -> String {
        ByteSize::b(self.size).to_string()
    }

    /// Check if this is a directory (ends with slash)
    pub fn is_directory(&self) -> bool {
        self.path.is_directory()
    }
}

impl fmt::Display for ArchiveFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.path, self.format_size())
    }
}

/// Response from the files.json endpoint
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchiveContents {
    /// List of files in the archive
    pub files: Vec<ArchiveFile>,
}

impl ArchiveContents {
    /// Get the total number of files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get the total size of all files
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Format the total size as human-readable
    pub fn format_total_size(&self) -> String {
        ByteSize::b(self.total_size()).to_string()
    }

    /// Get files by extension
    pub fn files_by_extension(&self, extension: &str) -> Vec<&ArchiveFile> {
        self.files
            .iter()
            .filter(|f| f.extension() == Some(extension))
            .collect()
    }

    /// Get files in a specific directory
    pub fn files_in_directory<P: AsRef<Path>>(&self, directory: P) -> Vec<&ArchiveFile> {
        let dir_path = directory.as_ref();
        self.files
            .iter()
            .filter(|f| f.directory() == Some(dir_path))
            .collect()
    }
}

/// Metadata response from the files.json endpoint containing preview link
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileMetadata {
    /// File ID
    pub file_id: u64,

    /// File name
    pub name: String,

    /// File version
    pub version: String,

    /// Size in bytes
    pub size_in_bytes: u64,

    /// Link to content preview JSON containing actual file structure
    pub content_preview_link: String,
}

/// Entry type in the file tree
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Directory,
}

/// Entry in the recursive file structure from content preview
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileTreeEntry {
    /// Full path of the file/directory
    pub path: String,

    /// Name of the file/directory
    pub name: String,

    /// Type: File or Directory
    #[serde(rename = "type")]
    pub entry_type: EntryType,

    /// Size (only for files, as human-readable string like "180.1 kB")
    pub size: Option<String>,

    /// Child entries (only for directories)
    pub children: Option<Vec<FileTreeEntry>>,
}

impl FileTreeEntry {
    /// Check if this entry is a file
    pub fn is_file(&self) -> bool {
        matches!(self.entry_type, EntryType::File)
    }

    /// Check if this entry is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.entry_type, EntryType::Directory)
    }

    /// Parse the size string to bytes
    /// Handles formats: "{X} B", "{X} kB", "{X} MB", "{X} GB"
    pub fn parse_size(&self) -> u64 {
        let size_str = match &self.size {
            Some(s) => s,
            None => return 0,
        };

        if let Some(b_pos) = size_str.find(" B") {
            // Handle "{X} B" format
            let num_str = &size_str[..b_pos];
            if let Ok(bytes) = num_str.parse::<f64>() {
                return bytes as u64;
            }
        }

        if let Some(kb_pos) = size_str.find(" kB") {
            // Handle "{X} kB" format
            let num_str = &size_str[..kb_pos];
            if let Ok(kb) = num_str.parse::<f64>() {
                return (kb * 1024.0) as u64;
            }
        }

        if let Some(mb_pos) = size_str.find(" MB") {
            // Handle "{X} MB" format
            let num_str = &size_str[..mb_pos];
            if let Ok(mb) = num_str.parse::<f64>() {
                return (mb * 1024.0 * 1024.0) as u64;
            }
        }

        if let Some(gb_pos) = size_str.find(" GB") {
            // Handle "{X} GB" format
            let num_str = &size_str[..gb_pos];
            if let Ok(gb) = num_str.parse::<f64>() {
                return (gb * 1024.0 * 1024.0 * 1024.0) as u64;
            }
        }

        // If no recognized format, return 0
        0
    }

    /// Flatten this entry and all its children into a list of files
    pub fn flatten_files(&self) -> Vec<ArchiveFile> {
        let mut files = Vec::new();
        self.collect_files(&mut files);
        files
    }

    fn collect_files(&self, files: &mut Vec<ArchiveFile>) {
        if self.is_file() {
            let size = self.parse_size();
            files.push(ArchiveFile::new(self.path.clone(), size, None));
        }

        if let Some(children) = &self.children {
            for child in children {
                child.collect_files(files);
            }
        }
    }
}

/// Response from the content preview link containing recursive file structure
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileTreeResponse {
    /// Root entries in the archive
    pub children: Vec<FileTreeEntry>,
}

impl FileTreeResponse {
    /// Convert to ArchiveContents by flattening the tree structure
    pub fn to_archive_contents(self) -> ArchiveContents {
        let mut files = Vec::new();

        for entry in self.children {
            files.extend(entry.flatten_files());
        }

        ArchiveContents { files }
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

    #[test]
    fn archive_file_path_operations() {
        let path = ArchiveFilePath::new("textures/armor/steel.dds");

        assert_eq!(path.file_name(), Some("steel.dds"));
        assert_eq!(path.extension(), Some("dds"));
        assert_eq!(path.parent(), Some(Path::new("textures/armor")));
        assert!(!path.is_directory());

        let dir = ArchiveFilePath::new("textures/");
        assert!(dir.is_directory());

        // Test deref functionality
        let path_ref: &Path = &path;
        assert_eq!(path_ref.file_name().unwrap().to_str().unwrap(), "steel.dds");
    }

    #[test]
    fn archive_file_operations() {
        let file = ArchiveFile::new("textures/armor/steel.dds", 1024, Some(1234567890));

        assert_eq!(file.file_name(), Some("steel.dds"));
        assert_eq!(file.extension(), Some("dds"));
        assert_eq!(file.directory(), Some(Path::new("textures/armor")));
        assert_eq!(file.format_size(), "1.0 KiB");
        assert!(!file.is_directory());

        let dir = ArchiveFile::new("textures/", 0, None);
        assert!(dir.is_directory());
    }

    #[test]
    fn archive_contents_operations() {
        let files = vec![
            ArchiveFile::new("mod.esp", 2048, None),
            ArchiveFile::new("textures/steel.dds", 1024, None),
            ArchiveFile::new("textures/iron.dds", 512, None),
            ArchiveFile::new("meshes/armor.nif", 4096, None),
        ];
        let contents = ArchiveContents { files };

        assert_eq!(contents.file_count(), 4);
        assert_eq!(contents.total_size(), 7680);
        assert_eq!(contents.format_total_size(), "7.5 KiB");

        let dds_files = contents.files_by_extension("dds");
        assert_eq!(dds_files.len(), 2);

        let texture_files = contents.files_in_directory("textures");
        assert_eq!(texture_files.len(), 2);
    }

    #[test]
    fn file_tree_entry_operations() {
        let file_entry = FileTreeEntry {
            path: "textures/armor/steel.dds".to_string(),
            name: "steel.dds".to_string(),
            entry_type: EntryType::File,
            size: Some("180.1 kB".to_string()),
            children: None,
        };

        assert!(file_entry.is_file());
        assert!(!file_entry.is_directory());

        let dir_entry = FileTreeEntry {
            path: "textures/".to_string(),
            name: "textures".to_string(),
            entry_type: EntryType::Directory,
            size: None,
            children: Some(vec![file_entry.clone()]),
        };

        assert!(!dir_entry.is_file());
        assert!(dir_entry.is_directory());

        // Test flattening
        let files = dir_entry.flatten_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.as_str(), "textures/armor/steel.dds");
    }

    #[test]
    fn size_parsing() {
        // Test bytes
        let entry_b = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: Some("1024 B".to_string()),
            children: None,
        };
        assert_eq!(entry_b.parse_size(), 1024);

        // Test kilobytes
        let entry_kb = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: Some("180.1 kB".to_string()),
            children: None,
        };
        assert_eq!(entry_kb.parse_size(), (180.1 * 1024.0) as u64);

        // Test megabytes
        let entry_mb = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: Some("5.5 MB".to_string()),
            children: None,
        };
        assert_eq!(entry_mb.parse_size(), (5.5 * 1024.0 * 1024.0) as u64);

        // Test gigabytes
        let entry_gb = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: Some("2.1 GB".to_string()),
            children: None,
        };
        assert_eq!(
            entry_gb.parse_size(),
            (2.1 * 1024.0 * 1024.0 * 1024.0) as u64
        );

        // Test no size
        let entry_none = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: None,
            children: None,
        };
        assert_eq!(entry_none.parse_size(), 0);

        // Test invalid format
        let entry_invalid = FileTreeEntry {
            path: "test.txt".to_string(),
            name: "test.txt".to_string(),
            entry_type: EntryType::File,
            size: Some("invalid size".to_string()),
            children: None,
        };
        assert_eq!(entry_invalid.parse_size(), 0);
    }

    #[test]
    fn file_tree_response_conversion() {
        let tree = FileTreeResponse {
            children: vec![
                FileTreeEntry {
                    path: "mod.esp".to_string(),
                    name: "mod.esp".to_string(),
                    entry_type: EntryType::File,
                    size: Some("50.2 kB".to_string()),
                    children: None,
                },
                FileTreeEntry {
                    path: "textures/".to_string(),
                    name: "textures".to_string(),
                    entry_type: EntryType::Directory,
                    size: None,
                    children: Some(vec![FileTreeEntry {
                        path: "textures/armor/steel.dds".to_string(),
                        name: "steel.dds".to_string(),
                        entry_type: EntryType::File,
                        size: Some("180.1 kB".to_string()),
                        children: None,
                    }]),
                },
            ],
        };

        let archive_contents = tree.to_archive_contents();
        assert_eq!(archive_contents.file_count(), 2);

        // Should have mod.esp and steel.dds
        let paths: Vec<&str> = archive_contents
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert!(paths.contains(&"mod.esp"));
        assert!(paths.contains(&"textures/armor/steel.dds"));
    }
}
