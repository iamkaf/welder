use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
struct Config {
    version: u32,
    pack: Pack,
    paths: Paths,
    inputs: Inputs,
    build: BuildConfig,
    preview: PreviewConfig,
    sheet: SheetConfig,
    grid: GridConfig,
    metadata: Option<MetadataConfig>,
    publish: Option<PublishConfig>,
}

#[derive(Debug, Deserialize)]
struct Pack {
    name: String,
    slug: String,
    author: String,
    brand: String,
    license: Option<String>,
    semver: String,
}

#[derive(Debug, Deserialize)]
struct Paths {
    input: PathBuf,
    dist: PathBuf,
    previews: PathBuf,
    exports: PathBuf,
    sheets: Option<PathBuf>,
    package: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Inputs {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BuildConfig {
    resolutions: Vec<u32>,
    filter: Option<String>,
    trim_transparent: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PreviewConfig {
    styles: Vec<String>,
    background: String,
    scale: Option<u32>,
    watermark: Option<WatermarkConfig>,
}

#[derive(Debug, Deserialize)]
struct WatermarkConfig {
    enabled: bool,
    text: Option<String>,
    opacity: Option<f32>,
    position: Option<String>,
    margin_px: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SheetConfig {
    max_width: u32,
    max_height: u32,
    padding_px: u32,
    sort: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GridConfig {
    cell_px: u32,
    padding_px: u32,
    columns: u32,
}

#[derive(Debug, Deserialize)]
struct MetadataConfig {
    readme_template: Option<PathBuf>,
    itch_template: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct PublishConfig {
    itch: Option<PublishItchConfig>,
}

#[derive(Debug, Deserialize)]
struct PublishItchConfig {
    enabled: bool,
    user: String,
    project: String,
    channel: String,
    butler_bin: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    std::env::set_current_dir(&cli.cwd)
        .with_context(|| format!("failed to change directory to {}", cli.cwd))?;

    if cli.verbose > 0 {
        eprintln!("cwd: {}", std::env::current_dir()?.display());
    }

    let config_path = PathBuf::from(&cli.config);

    match cli.command {
        Commands::Init {
            name,
            author,
            brand,
            input,
            yes,
        } => run_init(&config_path, name, author, brand, input, yes),
        Commands::Doctor { butler } => run_doctor(&config_path, butler),
        Commands::Build { .. } => {
            bail!("build not implemented yet")
        }
        Commands::Preview { .. } => {
            bail!("preview not implemented yet")
        }
        Commands::Package { .. } => {
            bail!("package not implemented yet")
        }
        Commands::Publish { .. } => {
            bail!("publish not implemented yet")
        }
    }
}

fn run_init(
    config_path: &Path,
    name: Option<String>,
    author: Option<String>,
    brand: Option<String>,
    input: String,
    _yes: bool,
) -> Result<()> {
    let pack_name = name.unwrap_or_else(|| "New Asset Pack".to_string());
    let pack_slug = slugify(&pack_name);
    let pack_author = author.unwrap_or_else(|| "iamkaf".to_string());
    let pack_brand = brand.unwrap_or_else(|| pack_author.clone());

    if !config_path.exists() {
        let config = starter_config(&pack_name, &pack_slug, &pack_author, &pack_brand, &input);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(config_path, config)
            .with_context(|| format!("failed writing {}", config_path.display()))?;
        println!("created {}", config_path.display());
    } else {
        println!("exists  {}", config_path.display());
    }

    let cfg = load_config(config_path)?;
    let dirs = [
        cfg.paths.input.clone(),
        cfg.paths.dist.clone(),
        cfg.paths.previews.clone(),
        cfg.paths.exports.clone(),
        cfg.paths.package.clone(),
    ];

    for dir in dirs {
        fs::create_dir_all(&dir).with_context(|| format!("failed creating {}", dir.display()))?;
    }

    println!("initialized project folders");
    Ok(())
}

fn run_doctor(config_path: &Path, only_butler: bool) -> Result<()> {
    if only_butler {
        check_butler("butler")?;
        println!("doctor: OK (butler)");
        return Ok(());
    }

    let cfg = load_config(config_path)?;
    let mut issues = Vec::new();

    validate_config(&cfg, &mut issues);

    let checked_paths = [
        (&cfg.paths.input, true),
        (&cfg.paths.dist, false),
        (&cfg.paths.previews, false),
        (&cfg.paths.exports, false),
        (&cfg.paths.package, false),
    ];

    for (path, required_existing) in checked_paths {
        if !path.exists() {
            if required_existing {
                issues.push(format!("missing required path: {}", path.display()));
            } else {
                println!(
                    "missing path (will be created by commands): {}",
                    path.display()
                );
            }
        }
    }

    if let Some(publish) = &cfg.publish {
        if let Some(itch) = &publish.itch {
            if itch.enabled {
                let bin = itch.butler_bin.as_deref().unwrap_or("butler");
                if let Err(err) = check_butler(bin) {
                    issues.push(format!("butler check failed for '{}': {err:#}", bin));
                }
            }
        }
    }

    if issues.is_empty() {
        println!("doctor: OK");
        return Ok(());
    }

    eprintln!("doctor: found {} issue(s)", issues.len());
    for issue in issues {
        eprintln!("- {issue}");
    }
    bail!("doctor failed")
}

fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed reading config {}", path.display()))?;
    let cfg: Config = toml::from_str(&content)
        .with_context(|| format!("failed parsing TOML from {}", path.display()))?;
    Ok(cfg)
}

fn validate_config(cfg: &Config, issues: &mut Vec<String>) {
    if cfg.version != 1 {
        issues.push(format!(
            "unsupported config version {}, expected 1",
            cfg.version
        ));
    }
    if cfg.pack.name.trim().is_empty() {
        issues.push("pack.name is required".to_string());
    }
    if cfg.pack.slug.trim().is_empty() {
        issues.push("pack.slug is required".to_string());
    }
    if cfg.pack.semver.trim().is_empty() {
        issues.push("pack.semver is required".to_string());
    }
    if cfg.paths.input.as_os_str().is_empty() {
        issues.push("paths.input is required".to_string());
    }
    if cfg.build.resolutions.is_empty() {
        issues.push("build.resolutions must not be empty".to_string());
    }
    if cfg.grid.columns == 0 {
        issues.push("grid.columns must be > 0".to_string());
    }
}

fn check_butler(bin: &str) -> Result<()> {
    let status = Command::new(bin)
        .arg("--version")
        .status()
        .with_context(|| format!("failed to execute '{}'", bin))?;

    if !status.success() {
        bail!("'{} --version' exited with status {}", bin, status);
    }

    Ok(())
}

fn starter_config(name: &str, slug: &str, author: &str, brand: &str, input: &str) -> String {
    format!(
        "version = 1\n\n[pack]\nname = \"{name}\"\nslug = \"{slug}\"\nauthor = \"{author}\"\nbrand = \"{brand}\"\nlicense = \"CC0-1.0\"\nsemver = \"0.1.0\"\n\n[paths]\ninput = \"{input}\"\ndist = \"dist\"\npreviews = \"dist/previews\"\nexports = \"dist/exports\"\nsheets  = \"dist/sheets\"\npackage = \"dist/package\"\n\n[inputs]\ninclude = [\"**/*.png\"]\nexclude = [\"**/_wip/**\", \"**/.trash/**\"]\n\n[build]\nresolutions = [1, 2, 4]\nfilter = \"nearest\"\ntrim_transparent = true\n\n[preview]\nstyles = [\"sheet\", \"grid\"]\nbackground = \"#141414\"\nscale = 2\n\n[preview.watermark]\nenabled = true\ntext = \"iamkaf\"\nopacity = 0.12\nposition = \"bottom-right\"\nmargin_px = 12\n\n[sheet]\nmax_width = 2048\nmax_height = 2048\npadding_px = 2\nsort = \"name\"\n\n[grid]\ncell_px = 64\npadding_px = 8\ncolumns = 8\n\n[metadata]\nreadme_template = \"templates/README.md.tmpl\"\nitch_template = \"templates/ITCH.md.tmpl\"\n\n[publish.itch]\nenabled = true\nuser = \"{author}\"\nproject = \"{slug}\"\nchannel = \"default\"\nbutler_bin = \"butler\"\n"
    )
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for ch in s.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
