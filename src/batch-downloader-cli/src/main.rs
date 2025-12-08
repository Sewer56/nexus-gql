//! # Batch Downloader CLI
//!
//! This is buggy, messy, mostly LLM generated code for one time use only.
//! Don't use any of this in production, it's bad.
//!
//! Command-line interface for batch downloading and analyzing mods from Nexus Mods using the GraphQL API.
//!
//! ## Features
//!
//! - **Mod Size Analysis**: Calculate total size of mods in a category
//! - **File Type Breakdown**: Detailed statistics for all file types found within archives
//! - **File Extension Filtering**: Filter mods by file extensions within archives
//! - **Archive Content Inspection**: Deep inspection of mod archive contents using the legacy REST API
//! - **Concurrent Processing**: Configurable concurrency for faster analysis
//! - **Authentication**: Support for authenticated requests via API key
//! - **Download and Extraction**: Actually download and extract mod files with filtering support
//!
//! ## Download and Extraction
//!
//! The tool now supports downloading and extracting mod files in addition to analysis:
//! - Use `--download` flag to enable download mode
//! - Specify `--output-path` to set where files should be downloaded and extracted
//! - Extraction happens immediately after each download with the same concurrency as `--concurrency`
//! - Requires 7-zip CLI (`7z` command) to be installed for extraction
//! - Archives are automatically deleted after successful extraction
//! - If file extension filtering is enabled, only matching files are kept while preserving directory structure
//!
//! ## Archive Content Inspection
//!
//! The tool now always inspects archive contents to provide detailed file type breakdowns. This means:
//! - Every mod's archive contents are analyzed for comprehensive statistics
//! - File type breakdown shows total size, count, and average size for each extension
//! - Processing is slower compared to basic file analysis due to additional API calls
//! - Results provide much more detailed insights into mod compositions
//!
//! ## File Extension Filtering
//!
//! The `--file-extension` option enables filtering mods based on files contained within their archives.
//! This feature:
//! - Uses the `get_mod_file_contents` method to inspect archive contents before downloading
//! - Returns only mods where at least one archive contains a file with the specified extension
//! - **When downloading**: Only downloads and extracts files that contain the target extension (saves bandwidth!)
//! - Requires an API key for archive content inspection
//! - May be slower than basic file analysis due to additional API calls
//! - When used with `--download`, only files matching the extension are extracted
//!
//! Example: `--file-extension dds` will only include mods that contain .dds texture files, and when downloading, will only download those specific files containing textures.
//!
//! ### Risky Method for Archive Inspection
//!
//! The `--use-risky-method` flag enables a faster but potentially less reliable method for archive content inspection:
//! - **Benefits**: ~50% faster, saves one HTTP request per file, no API key required for content inspection
//! - **Method**: Uses the URI field from GetModFiles query directly instead of fetching metadata first
//! - **Risks**: May break if Nexus Mods changes their internal URL structure
//! - **Fallback**: Automatically falls back to standard method if URI is empty
//!
//! This method is recommended for large batch operations where speed is important.
//!
//! ### Sample Usage
//!
//! ```bash
//! # Analyze only (default)
//! batch-downloader-cli mod-sizes --game skyrimspecialedition --category "Models and Textures" --count 100
//!
//! # Download and extract all files (concurrency controls both download and extraction)
//! batch-downloader-cli mod-sizes --game skyrimspecialedition --category "Models and Textures" --count 10 --download --output-path ./downloads --concurrency 6
//!
//! # Download only mods containing .dds files, extract only .dds files
//! batch-downloader-cli mod-sizes --game skyrimspecialedition --category "Models and Textures" --count 10 --download --output-path ./downloads --file-extension dds
//! ```
//!
//! ### Sample Output with File Extension Filter
//!
//! ```text
//! 📊 SUMMARY
//!═══════════════════════════════════════
//!🎮 Game: skyrimspecialedition (ID: 1704)
//!📂 Category:
//!📊 Total mods analyzed: 100
//!🔍 File extension filter: .dds
//!✅ Mods matching filter: 30
//!
//!📋 ALL MODS STATISTICS
//!─────────────────────────────────────
//!📁 Mods with files: 100
//!📄 Total files: 149
//!💾 Combined size: 52.4 GiB
//!📈 Average mod size: 536.3 MiB
//!
//!🔍 ARCHIVE INSPECTED STATISTICS
//!─────────────────────────────────────
//!📁 Mods with archive contents inspected: 99
//!📁 Archive inspected mods with files: 99
//!📄 Archive inspected total files: 148
//!💾 Archive inspected combined size: 52.2 GiB
//!📈 Archive inspected average mod size: 540.3 MiB
//!
//!🎯 FILTERED MODS STATISTICS
//!─────────────────────────────────────
//!📁 Filtered mods with files: 30
//!📄 Filtered total files: 48
//!💾 Filtered combined size: 16.5 GiB
//!📈 Filtered average mod size: 562.4 MiB
//!
//!📄 FILE TYPE BREAKDOWN
//!══════════════════════════════════════════════════════════════════
//!Extension  Item Count      Total Size      Average Size   
//!──────────────────────────────────────────────────────────────────
//!.dds       17974           64.4 GiB        3.7 MiB        
//!.bsa       68              13.5 GiB        203.3 MiB      
//!.nif       16369           9.6 GiB         611.8 KiB      
//!.tri       3072            1.8 GiB         609.5 KiB      
//!.pdb       69              1.7 GiB         25.8 MiB    
//! ```

use clap::{Parser, Subcommand};
use futures::{stream, StreamExt, TryStreamExt};
use nexus_gql::{
    get_game, get_mod_files, get_popular_mods_for_game_and_category_by_endorsements_descending,
    types::ByteSizeString, GetGame, GetModFiles,
    GetPopularModsForGameAndCategoryByEndorsementsDescending, NexusClient,
};
use std::{
    collections::HashMap,
    env,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
};
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
    process::Command,
};

/// Statistics for a group of mods
#[derive(Debug, Clone, Default)]
pub struct ModStatistics {
    /// Total number of mods in this group
    pub total_mods: usize,
    /// Number of mods with files
    pub mods_with_files: usize,
    /// Total number of files across all mods
    pub total_files: usize,
    /// Total size of all files across all mods
    pub total_size: u64,
}

impl ModStatistics {
    /// Add statistics from a single mod to this group
    pub fn add_mod(&mut self, has_files: bool, file_count: usize, total_size: u64) {
        self.total_mods += 1;
        if has_files {
            self.mods_with_files += 1;
            self.total_files += file_count;
            self.total_size += total_size;
        }
    }

    /// Calculate average mod size for mods with files
    pub fn average_mod_size(&self) -> Option<u64> {
        if self.mods_with_files > 0 {
            Some(self.total_size / self.mods_with_files as u64)
        } else {
            None
        }
    }
}

/// Result of processing a single mod's files
#[derive(Debug, Clone)]
pub struct ModProcessingResult {
    /// The mod information
    pub mod_info: get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    /// Number of mods with files (0 or 1 for individual mods)
    pub mods_with_files: usize,
    /// Total number of files processed
    pub total_files: usize,
    /// Total size of all files
    pub total_size: u64,
    /// Whether this mod matched the extension filter (if any)
    pub matched_filter: bool,
    /// Whether we successfully obtained archive contents for this mod
    pub archive_inspected: bool,
    /// File extension statistics from archive contents
    pub file_extension_stats: FileExtensionStatistics,
}

/// Statistics for file extensions
#[derive(Debug, Clone, Default)]
pub struct FileExtensionStatistics {
    /// Map of extension to (count, total_size)
    pub extensions: HashMap<String, (usize, u64)>,
}

impl FileExtensionStatistics {
    /// Add a file with the given extension and size
    pub fn add_file(&mut self, extension: String, size: u64) {
        let entry = self.extensions.entry(extension).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += size;
    }

    /// Merge another FileExtensionStatistics into this one
    pub fn merge(&mut self, other: &FileExtensionStatistics) {
        for (ext, (count, size)) in &other.extensions {
            let entry = self.extensions.entry(ext.clone()).or_insert((0, 0));
            entry.0 += count;
            entry.1 += size;
        }
    }

    /// Get sorted list of extensions by total size descending
    pub fn sorted_by_size(&self) -> Vec<(&String, &(usize, u64))> {
        let mut extensions: Vec<_> = self.extensions.iter().collect();
        extensions.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
        extensions
    }
}

#[derive(Parser)]
#[command(name = "batch-downloader-cli")]
#[command(about = "Batch downloader for Nexus Mods using GraphQL API")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get the total size of the first 1000 mods for a given game and category
    ModSizes {
        /// Game identifier - can be either domain name (e.g., "skyrimspecialedition") or game ID (e.g., "1704")
        #[arg(short, long)]
        game: String,

        /// Category name (e.g., "Models and Textures", "Gameplay", "Weapons", etc.)
        #[arg(short, long)]
        category: String,

        /// Number of mods to analyze (default: 1000, max: 1000)
        #[arg(short = 'n', long, default_value = "1000")]
        count: i64,

        /// Only include main files when calculating sizes
        #[arg(long)]
        main_files_only: bool,

        /// Number of concurrent requests to make (default: 4, max: 20)
        #[arg(short = 'j', long, default_value = "4")]
        concurrency: usize,

        /// Optional API key for authenticated requests (can also be set via NEXUS_API_KEY environment variable)
        #[arg(long)]
        api_key: Option<String>,

        /// Filter mods by file extension - only include mods that contain files with this extension
        /// (e.g., "dds", "esp", "esm"). This requires archive content inspection and may be slower.
        #[arg(long)]
        file_extension: Option<String>,

        /// Use the risky method for archive content inspection (faster but may be less reliable)
        /// This bypasses the official metadata API and constructs URLs directly using filenames.
        /// Benefits: ~50% faster, no API key required for content inspection.
        /// Risks: May break if Nexus changes their internal URL structure.
        #[arg(long)]
        use_risky_method: bool,

        /// Output path for downloaded and extracted files
        #[arg(short, long)]
        output_path: Option<String>,

        /// Actually download and extract files (default is analyze only)
        #[arg(long)]
        download: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    match args.command {
        Commands::ModSizes {
            game,
            category,
            count,
            main_files_only,
            concurrency,
            api_key,
            file_extension,
            use_risky_method,
            output_path,
            download,
        } => {
            handle_mod_sizes(
                game,
                category,
                count,
                main_files_only,
                concurrency,
                api_key,
                file_extension,
                use_risky_method,
                output_path,
                download,
            )
            .await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_mod_sizes(
    game: String,
    category: String,
    count: i64,
    main_files_only: bool,
    concurrency: usize,
    cli_api_key: Option<String>,
    file_extension: Option<String>,
    use_risky_method: bool,
    output_path: Option<String>,
    download: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate concurrency parameter
    let concurrency = if concurrency == 0 {
        1
    } else if concurrency > 20 {
        println!("⚠️ Concurrency limited to 20 to avoid overwhelming the API");
        20
    } else {
        concurrency
    };

    // Validate download parameters
    if download {
        if output_path.is_none() {
            return Err("Output path is required when downloading files. Use --output-path".into());
        }

        // Check if 7z is available
        if (tokio::process::Command::new("7z")
            .arg("--help")
            .output()
            .await)
            .is_err()
        {
            return Err(
                "7z command not found. Please install 7-zip to use extraction functionality".into(),
            );
        }

        println!("🚀 Download mode enabled - files will be downloaded and extracted");
        println!("📂 Output path: {}", output_path.as_ref().unwrap());
        println!("🔧 Extraction concurrency: {concurrency}");
    } else {
        println!("🔍 Analysis mode - no files will be downloaded");
    }

    // Get API key from CLI argument or environment variable
    let api_key = cli_api_key
        .or_else(|| env::var("NEXUS_API_KEY").ok())
        .ok_or("API key is required. Set NEXUS_API_KEY environment variable or use --api-key")?;

    // Create client with API key
    let client = Arc::new(NexusClient::new().with_api_key(api_key));

    // Resolve game identifier to game ID
    let game_id = resolve_game_id(&client, &game).await?;

    println!("🎮 Analyzing mods for game ID: {game_id} in category: '{category}'");
    println!("📊 Getting first {count} popular mods...\n");

    // Get popular mods in batches (API allows 80 max per request [undocumented])
    let mut all_mods = Vec::new();
    let batch_size = 80;
    let mut offset = 0;

    while all_mods.len() < count as usize {
        let remaining = count as usize - all_mods.len();
        let current_batch_size = remaining.min(batch_size);

        let variables =
            get_popular_mods_for_game_and_category_by_endorsements_descending::Variables {
                game_id: game_id.clone(),
                category_name: category.clone(),
                count: Some(current_batch_size as i64),
                offset: Some(offset),
            };

        let response = client
            .execute::<GetPopularModsForGameAndCategoryByEndorsementsDescending>(variables)
            .await?;

        if response.mods.nodes.is_empty() {
            println!("⚠️ No more mods found. Got {} mods total.", all_mods.len());
            break;
        }

        all_mods.extend(response.mods.nodes);
        offset += current_batch_size as i64;

        println!("📥 Retrieved {} mods so far...", all_mods.len());
    }

    if all_mods.is_empty() {
        println!("❌ No mods found for game '{game}' in category '{category}'");
        return Ok(());
    }

    println!(
        "🔍 Found {} mods. Now analyzing file sizes with {} concurrent requests...\n",
        all_mods.len(),
        concurrency
    );

    // Set up progress tracking
    let completed_count = Arc::new(AtomicUsize::new(0));
    let total_mods = all_mods.len();

    // Create output directory if downloading
    if download {
        let output_dir = output_path.as_ref().unwrap();
        tokio::fs::create_dir_all(output_dir).await?;
        println!("📁 Created output directory: {output_dir}");
    }

    // Analyze mod file sizes concurrently
    let results = stream::iter(all_mods.iter())
        .map(|mod_info| {
            process_mod_files(
                Arc::clone(&client),
                game_id.clone(),
                mod_info.clone(),
                main_files_only,
                Arc::clone(&completed_count),
                total_mods,
                file_extension.clone(),
                use_risky_method,
                output_path.clone(),
                download,
            )
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    // Aggregate results using ModStatistics
    let mut all_stats = ModStatistics::default();
    let mut archive_inspected_stats = ModStatistics::default();
    let mut filtered_stats = ModStatistics::default();
    let mut file_extension_stats = FileExtensionStatistics::default();

    for result in results {
        let has_files = result.mods_with_files > 0;

        // All mods statistics
        all_stats.add_mod(has_files, result.total_files, result.total_size);

        // Archive inspected mods statistics
        if result.archive_inspected {
            archive_inspected_stats.add_mod(has_files, result.total_files, result.total_size);
        }

        // Filtered mods statistics
        if result.matched_filter {
            filtered_stats.add_mod(has_files, result.total_files, result.total_size);
        }

        // File extension statistics
        file_extension_stats.merge(&result.file_extension_stats);
    }

    print_summary(
        &game,
        &game_id,
        &category,
        all_mods.len(),
        &all_stats,
        &file_extension,
        &archive_inspected_stats,
        &filtered_stats,
        &file_extension_stats,
    );

    Ok(())
}

/// Process files for a single mod and return aggregated statistics
#[allow(clippy::too_many_arguments)]
async fn process_mod_files(
    client: Arc<NexusClient>,
    game_id: String,
    mod_info: get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    main_files_only: bool,
    completed_count: Arc<AtomicUsize>,
    total_mods: usize,
    file_extension: Option<String>,
    use_risky_method: bool,
    output_path: Option<String>,
    download: bool,
) -> ModProcessingResult {
    // Get mod files
    let variables = get_mod_files::Variables {
        mod_id: mod_info.mod_id.to_string(),
        game_id,
    };

    let result = client.execute::<GetModFiles>(variables).await;

    // Update progress
    let completed = completed_count.fetch_add(1, Ordering::Relaxed) + 1;

    match result {
        Ok(files_response) => {
            let filtered_files = if main_files_only {
                files_response
                    .mod_files
                    .iter()
                    .filter(|file| file.category == get_mod_files::ModFileCategory::MAIN)
                    .collect::<Vec<_>>()
            } else {
                files_response.mod_files.iter().collect::<Vec<_>>()
            };

            let mod_size: u64 = filtered_files
                .iter()
                .filter_map(|file| file.size_in_bytes.as_ref())
                .map(|size| size.bytes())
                .sum();

            // Always get archive contents for file extension breakdown
            let (
                matched_file_extension,
                archive_inspected,
                file_extension_stats,
                files_to_download,
            ) = get_archive_contents_and_check_filter(
                &client,
                &mod_info,
                &filtered_files,
                file_extension.as_ref(),
                use_risky_method,
            )
            .await;

            // Download and extract files if requested
            if let (true, Some(output_path_ref)) = (download, output_path.as_ref()) {
                let skipped_count = filtered_files.len() - files_to_download.len();
                if skipped_count > 0 {
                    println!(
                        "⏭️ Skipping {skipped_count} files that don't contain target extension"
                    );
                }

                if let Err(e) = download_and_extract_mod_files(
                    &client,
                    &mod_info,
                    &files_to_download,
                    output_path_ref,
                    file_extension.as_ref(),
                )
                .await
                {
                    println!("❌ Failed to download/extract mod {}: {e}", mod_info.name);
                }
            }

            if !filtered_files.is_empty() {
                print_mod_processing_success(
                    completed,
                    total_mods,
                    &mod_info.name,
                    &mod_info.mod_id.to_string(),
                    filtered_files.len(),
                    mod_size,
                    main_files_only,
                    matched_file_extension,
                );

                ModProcessingResult {
                    mod_info: mod_info.clone(),
                    mods_with_files: 1,
                    total_files: filtered_files.len(),
                    total_size: mod_size,
                    matched_filter: matched_file_extension,
                    archive_inspected,
                    file_extension_stats,
                }
            } else {
                print_mod_no_files(
                    completed,
                    total_mods,
                    &mod_info.name,
                    &mod_info.mod_id.to_string(),
                    main_files_only,
                );

                ModProcessingResult {
                    mod_info: mod_info.clone(),
                    mods_with_files: 0,
                    total_files: 0,
                    total_size: 0,
                    matched_filter: matched_file_extension,
                    archive_inspected,
                    file_extension_stats,
                }
            }
        }
        Err(e) => {
            print_mod_processing_error(
                completed,
                total_mods,
                &mod_info.name,
                &mod_info.mod_id.to_string(),
                &e.to_string(),
            );
            ModProcessingResult {
                mod_info: mod_info.clone(),
                mods_with_files: 0,
                total_files: 0,
                total_size: 0,
                matched_filter: false,
                archive_inspected: false,
                file_extension_stats: FileExtensionStatistics::default(),
            }
        }
    }
}

/// Get archive contents and collect file extension statistics, also checking if filter matches
/// Returns (matched_filter, archive_inspected, file_extension_stats, files_to_download) tuple
async fn get_archive_contents_and_check_filter(
    client: &NexusClient,
    mod_info: &get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    files: &[&get_mod_files::GetModFilesModFiles],
    target_extension: Option<&String>,
    use_risky_method: bool,
) -> (
    bool,
    bool,
    FileExtensionStatistics,
    Vec<get_mod_files::GetModFilesModFiles>,
) {
    let game_domain = &mod_info.game.domain_name;
    let mod_id = &mod_info.mod_id.to_string();
    let game_id = &mod_info.game.id.to_string();

    let mut archive_inspected = false;
    let mut matched_filter = true; // Default to true if no filter is specified
    let mut file_extension_stats = FileExtensionStatistics::default();
    let mut files_to_download = Vec::new();

    for file in files {
        let file_id = &file.file_id.to_string();
        let mut file_contains_target = false;

        // Try to get archive contents for this file
        let archive_contents_result = if use_risky_method {
            // Use the risky method with the URI field as filename
            if !file.uri.is_empty() {
                client
                    .get_mod_file_contents_risky(game_id, mod_id, &file.uri)
                    .await
            } else {
                // Fall back to the standard method if URI is empty
                client
                    .get_mod_file_contents(game_domain, mod_id, file_id)
                    .await
            }
        } else {
            // Use the standard method
            client
                .get_mod_file_contents(game_domain, mod_id, file_id)
                .await
        };

        match archive_contents_result {
            Ok(archive_contents) => {
                archive_inspected = true;

                // Collect file extension statistics
                for archive_file in &archive_contents.files {
                    if let Some(extension) = archive_file.extension() {
                        // Use the size from archive file
                        let file_size = archive_file.size;
                        file_extension_stats.add_file(extension.to_string(), file_size);
                    }
                }

                // Check if any file in the archive has the target extension (if filter is specified)
                if let Some(target_ext) = target_extension {
                    let has_extension = archive_contents
                        .files
                        .iter()
                        .any(|archive_file| archive_file.extension() == Some(target_ext));

                    file_contains_target = has_extension;
                    if !has_extension {
                        matched_filter = false;
                    }
                }

                // Decide whether to download this file
                if target_extension.is_some() {
                    // If filtering is enabled, only download files that contain the target extension
                    if file_contains_target {
                        files_to_download.push((*file).clone());
                    }
                } else {
                    // No filter, download all files
                    files_to_download.push((*file).clone());
                }
            }
            Err(_) => {
                // If we can't get archive contents, skip this file for filtering but include for download if no filter
                if target_extension.is_none() {
                    files_to_download.push((*file).clone());
                }
                continue;
            }
        }
    }

    // If we have a filter but no archives were inspected, consider it not matched
    if target_extension.is_some() && !archive_inspected {
        matched_filter = false;
    }

    (
        matched_filter,
        archive_inspected,
        file_extension_stats,
        files_to_download,
    )
}

/// Print successful mod processing result
#[allow(clippy::too_many_arguments)]
fn print_mod_processing_success(
    completed: usize,
    total_mods: usize,
    mod_name: &str,
    mod_id: &str,
    file_count: usize,
    mod_size: u64,
    main_files_only: bool,
    matched_filter: bool,
) {
    let size_str = if mod_size > 0 {
        ByteSizeString::from_u64(mod_size).format_bytes()
    } else {
        "Unknown size".to_string()
    };

    let filter_info = if main_files_only {
        " (main files only)"
    } else {
        ""
    };

    let filter_match_info = if matched_filter {
        " (matched filter)"
    } else {
        ""
    };

    println!(
        "📁 [{completed:4}/{total_mods}] {mod_name} (ID: {mod_id}): {file_count} files{filter_info}{filter_match_info}, {size_str} total"
    );
}

/// Print when no files are found for a mod
fn print_mod_no_files(
    completed: usize,
    total_mods: usize,
    mod_name: &str,
    mod_id: &str,
    main_files_only: bool,
) {
    let no_files_msg = if main_files_only {
        "No main files found"
    } else {
        "No files found"
    };

    println!("📁 [{completed:4}/{total_mods}] {mod_name} (ID: {mod_id}): {no_files_msg}");
}

/// Print error when mod processing fails
fn print_mod_processing_error(
    completed: usize,
    total_mods: usize,
    mod_name: &str,
    mod_id: &str,
    error_message: &str,
) {
    println!(
        "📁 [{completed:4}/{total_mods}] {mod_name} (ID: {mod_id}): ❌ Error: {error_message}"
    );
}

/// Print the summary of mod analysis results
#[allow(clippy::too_many_arguments)]
fn print_summary(
    game: &str,
    game_id: &str,
    category: &str,
    total_mods_analyzed: usize,
    all_stats: &ModStatistics,
    file_extension: &Option<String>,
    archive_inspected_stats: &ModStatistics,
    filtered_stats: &ModStatistics,
    file_extension_stats: &FileExtensionStatistics,
) {
    println!("\n📊 SUMMARY");
    println!("═══════════════════════════════════════");
    println!("🎮 Game: {game} (ID: {game_id})");
    println!("📂 Category: {category}");
    println!("📊 Total mods analyzed: {total_mods_analyzed}");

    if let Some(ext) = file_extension {
        println!("🔍 File extension filter: .{ext}");
        println!("✅ Mods matching filter: {}", filtered_stats.total_mods);
    }

    println!("\n📋 ALL MODS STATISTICS");
    println!("─────────────────────────────────────");
    println!("📁 Mods with files: {}", all_stats.mods_with_files);
    println!("📄 Total files: {}", all_stats.total_files);
    println!(
        "💾 Combined size: {}",
        ByteSizeString::from_u64(all_stats.total_size).format_bytes()
    );
    println!(
        "📈 Average mod size: {}",
        match all_stats.average_mod_size() {
            Some(avg) => ByteSizeString::from_u64(avg).format_bytes(),
            None => "N/A".to_string(),
        }
    );

    // Show archive inspection statistics if a filter was applied
    if file_extension.is_some() && archive_inspected_stats.total_mods > 0 {
        println!("\n🔍 ARCHIVE INSPECTED STATISTICS");
        println!("─────────────────────────────────────");
        println!(
            "📁 Mods with archive contents inspected: {}",
            archive_inspected_stats.total_mods
        );
        println!(
            "📁 Archive inspected mods with files: {}",
            archive_inspected_stats.mods_with_files
        );
        println!(
            "📄 Archive inspected total files: {}",
            archive_inspected_stats.total_files
        );
        println!(
            "💾 Archive inspected combined size: {}",
            ByteSizeString::from_u64(archive_inspected_stats.total_size).format_bytes()
        );
        println!(
            "📈 Archive inspected average mod size: {}",
            match archive_inspected_stats.average_mod_size() {
                Some(avg) => ByteSizeString::from_u64(avg).format_bytes(),
                None => "N/A".to_string(),
            }
        );
    }

    // Show filtered statistics if a filter was applied
    if file_extension.is_some() && filtered_stats.total_mods > 0 {
        println!("\n🎯 FILTERED MODS STATISTICS");
        println!("─────────────────────────────────────");
        println!(
            "📁 Filtered mods with files: {}",
            filtered_stats.mods_with_files
        );
        println!("📄 Filtered total files: {}", filtered_stats.total_files);
        println!(
            "💾 Filtered combined size: {}",
            ByteSizeString::from_u64(filtered_stats.total_size).format_bytes()
        );
        println!(
            "📈 Filtered average mod size: {}",
            match filtered_stats.average_mod_size() {
                Some(avg) => ByteSizeString::from_u64(avg).format_bytes(),
                None => "N/A".to_string(),
            }
        );
    }

    // Show file type breakdown
    if !file_extension_stats.extensions.is_empty() {
        println!("\n📄 FILE TYPE BREAKDOWN (Top 10)");
        println!("════════════════════════════════════════════════════════════════════════════");
        println!(
            "{:<10} {:<15} {:<15} {:<15} {:<10}",
            "Extension", "Item Count", "Total Size", "Average Size", "% of Total"
        );
        println!("────────────────────────────────────────────────────────────────────────────");

        // Calculate total size across all extensions for percentage calculation
        let total_all_extensions: u64 = file_extension_stats
            .extensions
            .values()
            .map(|(_, size)| size)
            .sum();

        for (extension, (count, total_size)) in
            file_extension_stats.sorted_by_size().iter().take(10)
        {
            let average_size = if *count > 0 {
                *total_size / *count as u64
            } else {
                0
            };
            let percentage = if total_all_extensions > 0 {
                (*total_size as f64 / total_all_extensions as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "{:<10} {:<15} {:<15} {:<15} {:<10}",
                format!(".{}", extension),
                count,
                ByteSizeString::from_u64(*total_size).format_bytes(),
                ByteSizeString::from_u64(average_size).format_bytes(),
                format!("{:.1}%", percentage)
            );
        }
    }
}

async fn resolve_game_id(
    client: &NexusClient,
    game_identifier: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check if it's already a numeric game ID
    if game_identifier.chars().all(|c| c.is_ascii_digit()) {
        return Ok(game_identifier.to_string());
    }

    // It's a domain name, resolve it to game ID
    println!("🔍 Resolving game domain '{game_identifier}' to game ID...");

    let variables = get_game::Variables {
        domain_name: game_identifier.to_string(),
    };

    let response = client.execute::<GetGame>(variables).await?;

    match response.game {
        Some(game) => {
            println!(
                "✅ Resolved '{}' to game: {} (ID: {})",
                game_identifier, game.name, game.id
            );
            Ok(game.id.to_string())
        }
        None => Err(format!("Game not found for domain: {game_identifier}").into()),
    }
}

/// Download and extract mod files for a single mod
async fn download_and_extract_mod_files(
    client: &NexusClient,
    mod_info: &get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    files: &[get_mod_files::GetModFilesModFiles],
    output_path: &str,
    file_extension_filter: Option<&String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_domain = &mod_info.game.domain_name;
    let mod_id = mod_info.mod_id.to_string();
    let mod_name = sanitize_filename(&mod_info.name);

    // Create mod directory
    let mod_dir = Path::new(output_path).join(&mod_name);
    if let Err(e) = tokio::fs::create_dir_all(&mod_dir).await {
        println!(
            "❌ Failed to create mod directory {}: {}",
            mod_dir.display(),
            e
        );
        return Ok(());
    }

    // Download and extract files sequentially for this mod
    for file in files {
        let client = client.clone();
        let game_domain = game_domain.clone();
        let mod_id = mod_id.clone();
        let file_id = file.file_id.to_string();
        let file_name = sanitize_filename(&file.name);

        // Create a separate folder for each file to avoid conflicts
        let file_dir = mod_dir.join(&file_name);
        let file_extension_filter = file_extension_filter.cloned();

        if let Err(e) = download_and_extract_single_file(
            &client,
            &game_domain,
            &mod_id,
            &file_id,
            &file_name,
            &file_dir,
            file_extension_filter.as_ref(),
        )
        .await
        {
            println!("❌ Download task failed: {e}");
        }

        // Clean up empty file directory if no files remain after processing
        if let Err(e) = cleanup_empty_directory(&file_dir).await {
            println!(
                "⚠️ Failed to cleanup file directory {}: {e}",
                file_dir.display()
            );
        }
    }

    // Clean up empty mod directory if no file directories remain
    if let Err(e) = cleanup_empty_directory(&mod_dir).await {
        println!(
            "⚠️ Failed to cleanup mod directory {}: {e}",
            mod_dir.display()
        );
    }

    Ok(())
}

/// Download and extract a single mod file
#[allow(clippy::too_many_arguments)]
async fn download_and_extract_single_file(
    client: &NexusClient,
    game_domain: &str,
    mod_id: &str,
    file_id: &str,
    file_name: &str,
    file_dir: &Path,
    file_extension_filter: Option<&String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create the file-specific directory
    if let Err(e) = tokio::fs::create_dir_all(file_dir).await {
        println!(
            "❌ Failed to create file directory {}: {}",
            file_dir.display(),
            e
        );
        return Ok(());
    }

    // Get download links
    let download_links = client.download_links(game_domain, mod_id, file_id).await?;

    if download_links.is_empty() {
        println!("⚠️ No download links available for file: {file_name}");
        return Ok(());
    }

    // Use the first available download link
    let download_url = &download_links[0].uri;

    // Create file path within the file-specific directory
    let file_path = file_dir.join(file_name);

    // Download the file
    println!("📥 Downloading: {file_name}");
    let response = reqwest::get(download_url).await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    // Save file to disk
    let mut file = File::create(&file_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.try_next().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    println!("✅ Downloaded: {file_name}");

    // Extract the file and remove archive regardless of extraction success
    let extraction_result = extract_archive(&file_path, file_dir, file_extension_filter).await;

    // Always remove the original archive file after download
    if file_path.exists() {
        if let Err(e) = remove_file(&file_path).await {
            println!(
                "⚠️ Failed to remove archive file {}: {e}",
                file_path.display()
            );
        }
    }

    // Return extraction result after cleanup
    extraction_result?;

    Ok(())
}

/// Extract an archive using 7z and optionally filter files
async fn extract_archive(
    archive_path: &Path,
    extract_to: &Path,
    file_extension_filter: Option<&String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Extract using 7z directly to destination
    println!(
        "🔧 Extracting: {}",
        archive_path.file_name().unwrap().to_string_lossy()
    );

    let output = Command::new("7z")
        .arg("x")
        .arg(archive_path)
        .arg("-y") // Assume yes for all prompts
        .arg(format!("-o{}", extract_to.display()))
        .output()
        .await?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("7z extraction failed: {error_msg}").into());
    }

    // If file extension filter is specified, remove non-matching files
    if let Some(target_extension) = file_extension_filter {
        filter_files_in_place(extract_to, target_extension).await?;
    }

    println!(
        "✅ Extracted: {}",
        archive_path.file_name().unwrap().to_string_lossy()
    );

    Ok(())
}

/// Remove files that don't match the extension filter while preserving directory structure
async fn filter_files_in_place(
    directory: &Path,
    target_extension: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut dirs_to_process = vec![directory.to_path_buf()];

    while let Some(current_dir) = dirs_to_process.pop() {
        let mut entries = tokio::fs::read_dir(&current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // Add subdirectory to processing queue
                dirs_to_process.push(path);
            } else if let Some(extension) = path.extension() {
                // Remove file if it doesn't match the target extension
                if extension.to_string_lossy().to_lowercase() != target_extension.to_lowercase() {
                    if let Err(e) = tokio::fs::remove_file(&path).await {
                        println!("⚠️ Failed to remove file {}: {e}", path.display());
                    }
                }
            } else {
                // Remove files without extensions if we're filtering
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    println!("⚠️ Failed to remove file {}: {e}", path.display());
                }
            }
        }
    }

    // Remove empty directories after filtering
    remove_empty_directories(directory).await?;

    Ok(())
}

/// Remove empty directories recursively
async fn remove_empty_directories(
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Collect all directories using iterative approach
    let mut all_dirs = Vec::new();
    let mut dirs_to_process = vec![directory.to_path_buf()];

    while let Some(current_dir) = dirs_to_process.pop() {
        let mut entries = tokio::fs::read_dir(&current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                all_dirs.push(path.clone());
                dirs_to_process.push(path);
            }
        }
    }

    // Sort by depth (deepest first) to ensure we remove subdirectories before parents
    all_dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    // Try to remove each directory if it's empty
    for dir_path in all_dirs {
        if dir_path != directory {
            // Don't remove the root directory
            if is_directory_empty(&dir_path).await? {
                if let Err(e) = tokio::fs::remove_dir(&dir_path).await {
                    println!(
                        "⚠️ Failed to remove empty directory {}: {e}",
                        dir_path.display()
                    );
                }
            }
        }
    }

    Ok(())
}

/// Check if a directory is empty
async fn is_directory_empty(
    directory: &Path,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let mut entries = tokio::fs::read_dir(directory).await?;
    Ok(entries.next_entry().await?.is_none())
}

/// Sanitize filename for filesystem use
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Clean up empty directory if no files remain after processing
async fn cleanup_empty_directory(
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut entries = tokio::fs::read_dir(directory).await?;

    // If directory is empty, remove it
    if entries.next_entry().await?.is_none() {
        tokio::fs::remove_dir(directory).await?;
    }

    Ok(())
}
