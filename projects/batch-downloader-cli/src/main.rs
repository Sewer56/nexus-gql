use clap::Parser;

#[derive(Parser)]
#[command(name = "nexus-cli")]
#[command(about = "List mod categories from Nexus Mods")]
struct Args {
    /// Game domain name (e.g., "skyrimspecialedition", "fallout4", "witcher3")
    #[arg(short, long, default_value = "skyrimspecialedition")]
    game: String,

    /// Include global categories
    #[arg(long)]
    global: bool,

    /// Optional API key for premium features
    #[arg(long)]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _args = Args::parse();

    Ok(())
}
