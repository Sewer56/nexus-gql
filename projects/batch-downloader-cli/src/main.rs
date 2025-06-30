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
        } => {
            handle_mod_sizes(game, category, count, main_files_only, concurrency, api_key).await?;
        }
    }

    Ok(())
}

async fn handle_mod_sizes(
    game: String,
    category: String,
    count: i64,
    main_files_only: bool,
    concurrency: usize,
    cli_api_key: Option<String>,
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
            )
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    // Aggregate results
    let mut total_size = 0u64;
    let mut mods_with_files = 0;
    let mut total_files = 0;

    for (mods_count, files_count, size) in results {
        mods_with_files += mods_count;
        total_files += files_count;
        total_size += size;
    }

    print_summary(
        &game,
        &game_id,
        &category,
        all_mods.len(),
        mods_with_files,
        total_files,
        total_size,
    );

    Ok(())
}

/// Process files for a single mod and return aggregated statistics
async fn process_mod_files(
    client: Arc<NexusClient>,
    game_id: String,
    mod_info: get_popular_mods_for_game_and_category_by_endorsements_descending::GetPopularModsForGameAndCategoryByEndorsementsDescendingModsNodes,
    main_files_only: bool,
    completed_count: Arc<AtomicUsize>,
    total_mods: usize,
) -> (usize, usize, u64) {
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
                    .filter(|file| file.category == crate::get_mod_files::ModFileCategory::MAIN)
                    .collect::<Vec<_>>()
            } else {
                files_response.mod_files.iter().collect::<Vec<_>>()
            };

            let mod_size: u64 = filtered_files
                .iter()
                .filter_map(|file| file.size_in_bytes.as_ref())
                .map(|size| size.bytes())
                .sum();

            if !filtered_files.is_empty() {
                print_mod_processing_success(
                    completed,
                    total_mods,
                    &mod_info.name,
                    &mod_info.mod_id.to_string(),
                    filtered_files.len(),
                    mod_size,
                    main_files_only,
                );

                (1, filtered_files.len(), mod_size) // (mods_with_files, total_files, total_size)
            } else {
                print_mod_no_files(
                    completed,
                    total_mods,
                    &mod_info.name,
                    &mod_info.mod_id.to_string(),
                    main_files_only,
                );

                (0, 0, 0)
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
            (0, 0, 0)
        }
    }
}

/// Print successful mod processing result
fn print_mod_processing_success(
    completed: usize,
    total_mods: usize,
    mod_name: &str,
    mod_id: &str,
    file_count: usize,
    mod_size: u64,
    main_files_only: bool,
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

    println!(
        "📁 [{completed:4}/{total_mods}] {mod_name} (ID: {mod_id}): {file_count} files{filter_info}, {size_str} total"
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
fn print_summary(
    game: &str,
    game_id: &str,
    category: &str,
    total_mods_analyzed: usize,
    mods_with_files: usize,
    total_files: usize,
    total_size: u64,
) {
    println!("\n📊 SUMMARY");
    println!("═══════════════════════════════════════");
    println!("🎮 Game: {game} (ID: {game_id})");
    println!("📂 Category: {category}");
    println!("📊 Total mods analyzed: {total_mods_analyzed}");
    println!("📁 Mods with files: {mods_with_files}");
    println!("📄 Total files: {total_files}");
    println!(
        "💾 Combined size: {}",
        ByteSizeString::from_u64(total_size).format_bytes()
    );
    println!(
        "📈 Average mod size: {}",
        if mods_with_files > 0 {
            ByteSizeString::from_u64(total_size / mods_with_files as u64).format_bytes()
        } else {
            "N/A".to_string()
        }
    );
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
