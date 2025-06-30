//! # Batch Downloader CLI
//!
//! Command-line interface for batch downloading and analyzing mods from Nexus Mods using the GraphQL API.
//!
//! ## Features
//!
//! - **Mod Size Analysis**: Calculate total size of mods in a category
//! - **File Extension Filtering**: Filter mods by file extensions within archives
//! - **Archive Content Inspection**: Deep inspection of mod archive contents using the legacy REST API
//! - **Concurrent Processing**: Configurable concurrency for faster analysis
//! - **Authentication**: Support for authenticated requests via API key
//!
//! ## File Extension Filtering
//!
//! The `--file-extension` option enables filtering mods based on files contained within their archives.
//! This feature:
//! - Uses the `get_mod_file_contents` method to inspect archive contents
//! - Returns only mods where at least one archive contains a file with the specified extension
//! - Requires an API key for archive content inspection
//! - May be slower than basic file analysis due to additional API calls
//!
//! Example: `--file-extension dds` will only include mods that contain .dds texture files.
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
//! ### Sample Output with File Extension Filter
//!
//! ```text
//! 📊 SUMMARY
//! ═══════════════════════════════════════
//! 🎮 Game: skyrimspecialedition (ID: 1704)
//! 📂 Category: Models and Textures
//! 📊 Total mods analyzed: 100
//! 🔍 File extension filter: .dds
//! ✅ Mods matching filter: 85
//!
//! 📋 ALL MODS STATISTICS
//! ─────────────────────────────────────
//! 📁 Mods with files: 95
//! 📄 Total files: 450
//! 💾 Combined size: 2.5 GiB
//! 📈 Average mod size: 26.9 MiB
//!
//! 🔍 ARCHIVE INSPECTED STATISTICS
//! ─────────────────────────────────────
//! 📁 Mods with archive contents inspected: 90
//! 📁 Archive inspected mods with files: 90
//! 📄 Archive inspected total files: 420
//! 💾 Archive inspected combined size: 2.4 GiB
//! 📈 Archive inspected average mod size: 27.2 MiB
//!
//! 🎯 FILTERED MODS STATISTICS
//! ─────────────────────────────────────
//! 📁 Filtered mods with files: 85
//! 📄 Filtered total files: 390
//! 💾 Filtered combined size: 2.2 GiB
//! 📈 Filtered average mod size: 26.5 MiB
//! ```

use clap::{Parser, Subcommand};
use futures::{stream, StreamExt};
use nexus_gql::{
    get_game, get_mod_files, get_popular_mods_for_game_and_category_by_endorsements_descending,
    types::ByteSizeString, GetGame, GetModFiles,
    GetPopularModsForGameAndCategoryByEndorsementsDescending, NexusClient,
};
use std::{
    env,
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
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
    /// Whether we successfully obtained archive contents for this mod (only relevant when file_extension filter is used)
    pub archive_inspected: bool,
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
            )
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    // Aggregate results using ModStatistics
    let mut all_stats = ModStatistics::default();
    let mut archive_inspected_stats = ModStatistics::default();
    let mut filtered_stats = ModStatistics::default();

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

            // Check file extension filter if specified
            let (matched_file_extension, archive_inspected) = if let Some(ext) = &file_extension {
                // Need to check archive contents for each file to see if any contains the target extension
                check_files_for_extension(
                    &client,
                    &mod_info,
                    &filtered_files,
                    ext,
                    use_risky_method,
                )
                .await
            } else {
                (true, false) // No filter means all match, but no archive inspection was done
            };

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
            }
        }
    }
}

/// Check if any of the mod files contain files with the specified extension
/// Returns (matched_filter, archive_inspected) tuple
async fn check_files_for_extension(
    client: &NexusClient,
    mod_info: &get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    files: &[&get_mod_files::GetModFilesModFiles],
    target_extension: &str,
    use_risky_method: bool,
) -> (bool, bool) {
    let game_domain = &mod_info.game.domain_name;
    let mod_id = &mod_info.mod_id.to_string();
    let game_id = &mod_info.game.id.to_string();

    let mut archive_inspected = false;
    let mut matched_filter = false;

    for file in files {
        let file_id = &file.file_id.to_string();

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

                // Check if any file in the archive has the target extension
                let has_extension = archive_contents
                    .files
                    .iter()
                    .any(|archive_file| archive_file.extension() == Some(target_extension));

                if has_extension {
                    matched_filter = true;
                    return (matched_filter, archive_inspected);
                }
            }
            Err(_) => {
                // If we can't get archive contents, skip this file
                // This might happen for files that don't have content preview available
                continue;
            }
        }
    }

    (matched_filter, archive_inspected)
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
