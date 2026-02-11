use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "welder")]
#[command(about = "Turn raw pixel art into ship-ready asset packs.", long_about = None)]
struct Cli {
    #[arg(short = 'C', long, default_value = ".")]
    cwd: String,

    #[arg(long, default_value = "welder.toml")]
    config: String,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scaffold welder.toml and project folders
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        brand: Option<String>,
        #[arg(long, default_value = "src")]
        input: String,
        #[arg(long)]
        yes: bool,
    },

    /// Verify environment, config, and external dependencies
    Doctor {
        #[arg(long)]
        butler: bool,
    },

    /// Build exports (resize, organize) into dist/
    Build {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        res: Option<String>,
        #[arg(long)]
        clean: bool,
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate preview images (sheet/grid)
    Preview {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, default_value = "both")]
        style: String,
        #[arg(long)]
        dry_run: bool,
    },

    /// Create a versioned zip in dist/package/
    Package {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        include_previews: bool,
    },

    /// Package + publish to itch.io via butler
    Publish {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    // NOTE: Skeleton only.
    // Next steps: implement config loading, pipeline steps, and butler integration.
    if cli.verbose > 0 {
        eprintln!("{cli:?}");
    }

    match cli.command {
        Commands::Init { .. } => {
            println!("welder init (skeleton): not implemented yet");
        }
        Commands::Doctor { .. } => {
            println!("welder doctor (skeleton): not implemented yet");
        }
        Commands::Build { .. } => {
            println!("welder build (skeleton): not implemented yet");
        }
        Commands::Preview { .. } => {
            println!("welder preview (skeleton): not implemented yet");
        }
        Commands::Package { .. } => {
            println!("welder package (skeleton): not implemented yet");
        }
        Commands::Publish { .. } => {
            println!("welder publish (skeleton): not implemented yet");
        }
    }
}
