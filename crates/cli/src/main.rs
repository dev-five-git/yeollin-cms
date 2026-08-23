//! Yeollin CLI
//!
//! Commands for building and developing Yeollin CMS applications.

mod commands;
mod template;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "yeollin")]
#[command(about = "Yeollin CMS CLI - Build type-safe CMS applications")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new plugin
    Init(commands::init::InitArgs),

    /// Prepare the frontend app by extracting template and linking plugins
    Prebuild(commands::prebuild::PrebuildArgs),

    /// Start development servers (prebuild + vinext dev + cargo watch)
    Dev(commands::dev::DevArgs),

    /// Build for production (prebuild + next export + cargo build)
    Build(commands::build::BuildArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        "yeollin=debug,info"
    } else {
        "yeollin=info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    match cli.command {
        Commands::Init(args) => commands::init::run(args).await,
        Commands::Prebuild(args) => commands::prebuild::run(args).await,
        Commands::Dev(args) => commands::dev::run(args).await,
        Commands::Build(args) => commands::build::run(args).await,
    }
}
