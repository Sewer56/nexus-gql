use clap::{Parser, Subcommand};
use nexus_gql::{
    get_game, get_mod_files, get_popular_mods_for_game_and_category_by_endorsements_descending,
    types::ByteSizeString, GetGame, GetModFiles,
    GetPopularModsForGameAndCategoryByEndorsementsDescending, NexusClient,
};
use std::env;

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
            api_key,
        } => {
            handle_mod_sizes(game, category, count, main_files_only, api_key).await?;
        }
    }

    Ok(())
}

async fn handle_mod_sizes(
    game: String,
    category: String,
    count: i64,
    main_files_only: bool,
    cli_api_key: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get API key from CLI argument or environment variable
    let api_key = cli_api_key
        .or_else(|| env::var("NEXUS_API_KEY").ok())
        .ok_or("API key is required. Set NEXUS_API_KEY environment variable or use --api-key")?;

    // Create client with API key
    let client = NexusClient::new().with_api_key(api_key);

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
        "🔍 Found {} mods. Now analyzing file sizes...\n",
        all_mods.len()
    );

    // Analyze mod file sizes
    let mut total_size = 0u64;
    let mut mods_with_files = 0;
    let mut total_files = 0;

    for (index, mod_info) in all_mods.iter().enumerate() {
        print!(
            "📁 [{:4}/{}] Analyzing mod: {} (ID: {})... ",
            index + 1,
            all_mods.len(),
            mod_info.name,
            mod_info.mod_id
        );

        // Get mod files
        let variables = get_mod_files::Variables {
            mod_id: mod_info.mod_id.to_string(),
            game_id: game_id.clone(),
        };

        match client.execute::<GetModFiles>(variables).await {
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
                    mods_with_files += 1;
                    total_files += filtered_files.len();
                    total_size += mod_size;

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
                        "{} files{}, {} total",
                        filtered_files.len(),
                        filter_info,
                        size_str
                    );
                } else {
                    let no_files_msg = if main_files_only {
                        "No main files found"
                    } else {
                        "No files found"
                    };
                    println!("{no_files_msg}");
                }
            }
            Err(e) => {
                println!("❌ Error: {e}");
            }
        }
    }

    // Print summary
    println!("\n📊 SUMMARY");
    println!("═══════════════════════════════════════");
    println!("🎮 Game: {game} (ID: {game_id})");
    println!("📂 Category: {category}");
    println!("📊 Total mods analyzed: {}", all_mods.len());
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

    Ok(())
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
