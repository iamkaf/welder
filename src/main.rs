use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use globset::{Glob, GlobSet, GlobSetBuilder};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImage, Rgba, RgbaImage};
use serde::Deserialize;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

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
        Commands::Build {
            profile: _,
            res,
            clean,
            dry_run,
        } => run_build(&config_path, res, clean, dry_run),
        Commands::Preview {
            profile: _,
            style,
            dry_run,
        } => run_preview(&config_path, &style, dry_run),
        Commands::Package {
            profile: _,
            out,
            include_previews,
        } => {
            let out = out.map(PathBuf::from);
            run_package(&config_path, out, include_previews).map(|_| ())
        }
        Commands::Publish {
            profile: _,
            channel,
            dry_run,
            yes: _,
        } => run_publish(&config_path, channel, dry_run),
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
        let butler_bin = if config_path.exists() {
            load_config(config_path)
                .ok()
                .and_then(|cfg| cfg.publish)
                .and_then(|p| p.itch)
                .and_then(|itch| itch.butler_bin)
                .unwrap_or_else(|| "butler".to_string())
        } else {
            "butler".to_string()
        };
        ensure_butler_available(&butler_bin)?;
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
                if let Err(err) = ensure_butler_available(bin) {
                    issues.push(format!("butler check failed for '{bin}': {err:#}"));
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

fn run_build(config_path: &Path, res: Option<String>, clean: bool, dry_run: bool) -> Result<()> {
    let cfg = load_config(config_path)?;
    let resolutions = parse_resolutions(res.as_deref(), &cfg.build.resolutions)?;

    if clean && cfg.paths.dist.exists() {
        if dry_run {
            println!("[dry-run] remove {}", cfg.paths.dist.display());
        } else {
            fs::remove_dir_all(&cfg.paths.dist)
                .with_context(|| format!("failed removing {}", cfg.paths.dist.display()))?;
        }
    }

    if dry_run {
        println!("[dry-run] create {}", cfg.paths.exports.display());
    } else {
        fs::create_dir_all(&cfg.paths.exports)
            .with_context(|| format!("failed creating {}", cfg.paths.exports.display()))?;
    }

    let input_files = collect_input_pngs(&cfg)?;
    if input_files.is_empty() {
        println!("no matching PNG files found");
        return Ok(());
    }

    for file in &input_files {
        let in_path = cfg.paths.input.join(file);
        let img = image::open(&in_path)
            .with_context(|| format!("failed reading image {}", in_path.display()))?;

        for factor in &resolutions {
            let out_path = cfg.paths.exports.join(format!("{factor}x")).join(file);

            if dry_run {
                println!("[dry-run] {} -> {}", in_path.display(), out_path.display());
                continue;
            }

            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed creating {}", parent.display()))?;
            }

            let scaled = if *factor == 1 {
                img.clone()
            } else {
                img.resize_exact(
                    img.width() * *factor,
                    img.height() * *factor,
                    FilterType::Nearest,
                )
            };

            scaled
                .save(&out_path)
                .with_context(|| format!("failed writing image {}", out_path.display()))?;
        }
    }

    println!("build: exported {} source file(s)", input_files.len());
    Ok(())
}

fn run_preview(config_path: &Path, style: &str, dry_run: bool) -> Result<()> {
    let cfg = load_config(config_path)?;
    let styles = preview_styles(style, &cfg.preview.styles)?;
    let sprites = load_sprites(&cfg)?;
    if sprites.is_empty() {
        bail!("no matching PNG files found for preview");
    }

    if dry_run {
        println!("[dry-run] create {}", cfg.paths.previews.display());
    } else {
        fs::create_dir_all(&cfg.paths.previews)
            .with_context(|| format!("failed creating {}", cfg.paths.previews.display()))?;
    }

    if styles.iter().any(|s| s == "sheet") {
        let out = cfg.paths.previews.join("sheet.png");
        if dry_run {
            println!("[dry-run] write {}", out.display());
        } else {
            let mut sheet = render_sheet(&cfg, &sprites)?;
            apply_watermark(&cfg, &mut sheet);
            sheet
                .save(&out)
                .with_context(|| format!("failed writing {}", out.display()))?;
        }
    }

    if styles.iter().any(|s| s == "grid") {
        let out = cfg.paths.previews.join("grid.png");
        if dry_run {
            println!("[dry-run] write {}", out.display());
        } else {
            let mut grid = render_grid(&cfg, &sprites)?;
            apply_watermark(&cfg, &mut grid);
            grid.save(&out)
                .with_context(|| format!("failed writing {}", out.display()))?;
        }
    }

    println!("preview: generated {}", styles.join(", "));
    Ok(())
}

fn run_package(
    config_path: &Path,
    out_path: Option<PathBuf>,
    include_previews: bool,
) -> Result<PathBuf> {
    let cfg = load_config(config_path)?;
    run_package_with_config(&cfg, out_path, include_previews)
}

fn run_package_with_config(
    cfg: &Config,
    out_path: Option<PathBuf>,
    include_previews: bool,
) -> Result<PathBuf> {
    if !cfg.paths.exports.exists() {
        bail!(
            "exports directory is missing at {} (run 'welder build' first)",
            cfg.paths.exports.display()
        );
    }

    fs::create_dir_all(&cfg.paths.package)
        .with_context(|| format!("failed creating {}", cfg.paths.package.display()))?;

    let out = out_path.unwrap_or_else(|| {
        cfg.paths
            .package
            .join(format!("{}-{}.zip", cfg.pack.slug, cfg.pack.semver))
    });

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating {}", parent.display()))?;
    }

    let file =
        fs::File::create(&out).with_context(|| format!("failed creating {}", out.display()))?;
    let mut zip = ZipWriter::new(file);
    let ts = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .map_err(|_| anyhow::anyhow!("failed creating fixed ZIP timestamp"))?;

    let file_opts = FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(ts)
        .unix_permissions(0o644);

    let mut export_files = collect_files_sorted(&cfg.paths.exports)?;
    for file in export_files.drain(..) {
        let rel = file
            .strip_prefix(&cfg.paths.exports)
            .with_context(|| format!("failed to relativize {}", file.display()))?;
        let zip_path = format!("exports/{}", normalize_for_glob(rel));
        let bytes =
            fs::read(&file).with_context(|| format!("failed reading {}", file.display()))?;
        zip.start_file(zip_path, file_opts)
            .context("failed starting zip file entry")?;
        zip.write_all(&bytes)
            .context("failed writing zip file entry")?;
    }

    if include_previews && cfg.paths.previews.exists() {
        let mut preview_files = collect_files_sorted(&cfg.paths.previews)?;
        for file in preview_files.drain(..) {
            let rel = file
                .strip_prefix(&cfg.paths.previews)
                .with_context(|| format!("failed to relativize {}", file.display()))?;
            let zip_path = format!("previews/{}", normalize_for_glob(rel));
            let bytes =
                fs::read(&file).with_context(|| format!("failed reading {}", file.display()))?;
            zip.start_file(zip_path, file_opts)
                .context("failed starting zip file entry")?;
            zip.write_all(&bytes)
                .context("failed writing zip file entry")?;
        }
    }

    if let Some(readme) = generate_readme_if_configured(cfg)? {
        zip.start_file("README.md", file_opts)
            .context("failed starting README entry")?;
        zip.write_all(readme.as_bytes())
            .context("failed writing README entry")?;
    }

    zip.finish().context("failed finalizing zip")?;

    println!("package: wrote {}", out.display());
    Ok(out)
}

fn run_publish(config_path: &Path, channel_override: Option<String>, dry_run: bool) -> Result<()> {
    let cfg = load_config(config_path)?;
    let itch = cfg
        .publish
        .as_ref()
        .and_then(|p| p.itch.as_ref())
        .context("publish.itch config is missing")?;

    if !itch.enabled {
        bail!("publish.itch.enabled is false");
    }

    let package_path = run_package_with_config(&cfg, None, false)?;
    let butler_bin = itch.butler_bin.as_deref().unwrap_or("butler");
    let channel = channel_override.unwrap_or_else(|| itch.channel.clone());
    let target = format!("{}/{}:{channel}", itch.user, itch.project);

    let cmd_preview = format!(
        "{} push {} {}",
        shell_escape(butler_bin),
        shell_escape(&package_path.to_string_lossy()),
        shell_escape(&target)
    );

    if dry_run {
        println!("publish dry-run command:");
        println!("{cmd_preview}");
        return Ok(());
    }

    ensure_butler_available(butler_bin)?;

    let status = Command::new(butler_bin)
        .arg("push")
        .arg(&package_path)
        .arg(&target)
        .status()
        .with_context(|| format!("failed to execute '{} push ...'", butler_bin))?;

    if !status.success() {
        bail!("publish failed: butler exited with status {status}");
    }

    println!("publish: pushed {}", package_path.display());
    Ok(())
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
    if cfg.inputs.include.is_empty() {
        issues.push("inputs.include must not be empty".to_string());
    }
    if cfg.sheet.max_width == 0 || cfg.sheet.max_height == 0 {
        issues.push("sheet.max_width and sheet.max_height must be > 0".to_string());
    }
    if cfg.grid.cell_px == 0 {
        issues.push("grid.cell_px must be > 0".to_string());
    }
    if let Some(filter) = &cfg.build.filter {
        if !filter.eq_ignore_ascii_case("nearest") {
            issues.push("build.filter must be 'nearest' for MVP".to_string());
        }
    }
}

fn ensure_butler_available(bin: &str) -> Result<()> {
    let result = Command::new(bin).arg("--version").status();
    match result {
        Ok(status) => {
            if status.success() {
                Ok(())
            } else {
                bail!("'{} --version' exited with status {}", bin, status)
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            bail!(
                "butler not found at '{}'. Install from https://itch.io/docs/butler/ and set publish.itch.butler_bin",
                bin
            )
        }
        Err(err) => Err(err).with_context(|| format!("failed to execute '{}'", bin)),
    }
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

fn parse_resolutions(override_value: Option<&str>, default_values: &[u32]) -> Result<Vec<u32>> {
    let mut values = if let Some(raw) = override_value {
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|token| {
                token
                    .parse::<u32>()
                    .with_context(|| format!("invalid resolution value '{token}'"))
            })
            .collect::<Result<Vec<u32>>>()?
    } else {
        default_values.to_vec()
    };

    values.sort_unstable();
    values.dedup();
    if values.is_empty() {
        bail!("no resolutions configured");
    }
    if values.contains(&0) {
        bail!("resolution factors must be > 0");
    }
    Ok(values)
}

fn collect_input_pngs(cfg: &Config) -> Result<Vec<PathBuf>> {
    if !cfg.paths.input.exists() {
        return Ok(Vec::new());
    }

    let include = build_globset(&cfg.inputs.include)?;
    let exclude = build_globset(&cfg.inputs.exclude)?;
    let mut files = Vec::new();

    for entry in WalkDir::new(&cfg.paths.input).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let abs = entry.path();
        let ext = abs
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("png"))
            .unwrap_or(false);
        if !ext {
            continue;
        }

        let rel = abs
            .strip_prefix(&cfg.paths.input)
            .with_context(|| format!("failed to strip prefix for {}", abs.display()))?;
        let rel_s = normalize_for_glob(rel);

        if !include.is_match(&rel_s) {
            continue;
        }
        if exclude.is_match(&rel_s) {
            continue;
        }
        files.push(PathBuf::from(rel_s));
    }

    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    Ok(files)
}

fn collect_files_sorted(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    Ok(files)
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).with_context(|| format!("invalid glob '{pattern}'"))?);
    }
    builder.build().context("failed to build glob matcher")
}

fn normalize_for_glob(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn preview_styles(style_arg: &str, config_default: &[String]) -> Result<Vec<String>> {
    let requested = if style_arg.eq_ignore_ascii_case("both") {
        vec!["sheet".to_string(), "grid".to_string()]
    } else {
        style_arg
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let mut styles = if requested.is_empty() {
        config_default
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>()
    } else {
        requested
    };

    styles.sort();
    styles.dedup();

    for style in &styles {
        if style != "sheet" && style != "grid" {
            bail!("unsupported preview style '{style}'");
        }
    }
    Ok(styles)
}

fn load_sprites(cfg: &Config) -> Result<Vec<(PathBuf, DynamicImage)>> {
    let files = collect_input_pngs(cfg)?;
    let mut sprites = Vec::with_capacity(files.len());
    for file in files {
        let abs = cfg.paths.input.join(&file);
        let img = image::open(&abs).with_context(|| format!("failed reading {}", abs.display()))?;
        sprites.push((file, img));
    }
    Ok(sprites)
}

fn render_sheet(cfg: &Config, sprites: &[(PathBuf, DynamicImage)]) -> Result<RgbaImage> {
    let mut placements = Vec::with_capacity(sprites.len());
    let mut x = cfg.sheet.padding_px;
    let mut y = cfg.sheet.padding_px;
    let mut row_h = 0u32;
    let mut max_x = 0u32;

    for (_, img) in sprites {
        let w = img.width();
        let h = img.height();

        if x > cfg.sheet.padding_px && x + w + cfg.sheet.padding_px > cfg.sheet.max_width {
            x = cfg.sheet.padding_px;
            y = y.saturating_add(row_h).saturating_add(cfg.sheet.padding_px);
            row_h = 0;
        }

        if y + h + cfg.sheet.padding_px > cfg.sheet.max_height {
            bail!(
                "sheet overflow: sprites exceed sheet.max_height ({})",
                cfg.sheet.max_height
            );
        }

        placements.push((x, y));
        max_x = max_x.max(x + w + cfg.sheet.padding_px);
        row_h = row_h.max(h);
        x = x.saturating_add(w).saturating_add(cfg.sheet.padding_px);
    }

    let height = if sprites.is_empty() {
        cfg.sheet.padding_px.saturating_mul(2).max(1)
    } else {
        y.saturating_add(row_h)
            .saturating_add(cfg.sheet.padding_px)
            .max(1)
    };
    let width = max_x.max(cfg.sheet.padding_px.saturating_mul(2)).max(1);
    let bg = parse_hex_color(&cfg.preview.background)?;
    let mut canvas = RgbaImage::from_pixel(width, height, bg);

    for ((_, img), (px, py)) in sprites.iter().zip(placements) {
        canvas
            .copy_from(&img.to_rgba8(), px, py)
            .context("failed placing sprite in sheet")?;
    }

    Ok(canvas)
}

fn render_grid(cfg: &Config, sprites: &[(PathBuf, DynamicImage)]) -> Result<RgbaImage> {
    let cell = cfg.grid.cell_px.max(1);
    let pad = cfg.grid.padding_px;
    let cols = cfg.grid.columns.max(1);
    let rows = ((sprites.len() as u32) + cols - 1) / cols;
    let width = cols
        .saturating_mul(cell)
        .saturating_add((cols + 1).saturating_mul(pad))
        .max(1);
    let height = rows
        .saturating_mul(cell)
        .saturating_add((rows + 1).saturating_mul(pad))
        .max(1);
    let bg = parse_hex_color(&cfg.preview.background)?;
    let mut canvas = RgbaImage::from_pixel(width, height, bg);

    for (idx, (_, img)) in sprites.iter().enumerate() {
        let i = idx as u32;
        let col = i % cols;
        let row = i / cols;
        let x0 = pad + col.saturating_mul(cell + pad);
        let y0 = pad + row.saturating_mul(cell + pad);
        let thumb = fit_in_cell(img, cell);
        let ox = x0 + (cell - thumb.width()) / 2;
        let oy = y0 + (cell - thumb.height()) / 2;
        canvas
            .copy_from(&thumb, ox, oy)
            .context("failed placing sprite in grid")?;
    }

    Ok(canvas)
}

fn fit_in_cell(img: &DynamicImage, cell_px: u32) -> RgbaImage {
    let w = img.width().max(1);
    let h = img.height().max(1);
    let scale = (cell_px as f32 / w as f32).min(cell_px as f32 / h as f32);
    let new_w = ((w as f32 * scale).floor() as u32).clamp(1, cell_px);
    let new_h = ((h as f32 * scale).floor() as u32).clamp(1, cell_px);
    img.resize_exact(new_w, new_h, FilterType::Nearest)
        .to_rgba8()
}

fn parse_hex_color(s: &str) -> Result<Rgba<u8>> {
    let value = s.trim().trim_start_matches('#');
    if value.len() != 6 {
        bail!("preview.background must be #RRGGBB, got '{s}'");
    }
    let r = u8::from_str_radix(&value[0..2], 16).with_context(|| format!("bad red in '{s}'"))?;
    let g = u8::from_str_radix(&value[2..4], 16).with_context(|| format!("bad green in '{s}'"))?;
    let b = u8::from_str_radix(&value[4..6], 16).with_context(|| format!("bad blue in '{s}'"))?;
    Ok(Rgba([r, g, b, 255]))
}

fn apply_watermark(cfg: &Config, image: &mut RgbaImage) {
    let wm = match cfg.preview.watermark.as_ref() {
        Some(wm) if wm.enabled => wm,
        _ => return,
    };

    let text = wm
        .text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("iamkaf");
    let opacity = wm.opacity.unwrap_or(0.12).clamp(0.0, 1.0);
    let margin = wm.margin_px.unwrap_or(12);
    let position = wm.position.as_deref().unwrap_or("bottom-right");
    draw_bitmap_text(image, text, opacity, position, margin);
}

fn draw_bitmap_text(image: &mut RgbaImage, text: &str, opacity: f32, position: &str, margin: u32) {
    let scale = 2u32;
    let glyph_w = 5u32;
    let glyph_h = 7u32;
    let spacing = 1u32;
    let chars: Vec<char> = text.to_ascii_uppercase().chars().collect();
    if chars.is_empty() {
        return;
    }

    let text_w = (chars.len() as u32)
        .saturating_mul(glyph_w + spacing)
        .saturating_sub(spacing)
        .saturating_mul(scale);
    let text_h = glyph_h.saturating_mul(scale);

    let (mut x, y) = match position.to_ascii_lowercase().as_str() {
        "top-left" | "tl" => (margin, margin),
        "top-right" | "tr" => (image.width().saturating_sub(text_w + margin), margin),
        "bottom-left" | "bl" => (margin, image.height().saturating_sub(text_h + margin)),
        "center" => (
            (image.width().saturating_sub(text_w)) / 2,
            (image.height().saturating_sub(text_h)) / 2,
        ),
        _ => (
            image.width().saturating_sub(text_w + margin),
            image.height().saturating_sub(text_h + margin),
        ),
    };

    let alpha = (opacity * 255.0).round().clamp(0.0, 255.0) as u8;
    for ch in chars {
        draw_glyph(image, ch, x, y, scale, alpha);
        x = x.saturating_add((glyph_w + spacing).saturating_mul(scale));
    }
}

fn draw_glyph(image: &mut RgbaImage, ch: char, x: u32, y: u32, scale: u32, alpha: u8) {
    let pattern = glyph_pattern(ch);
    for (row, bits) in pattern.iter().enumerate() {
        for (col, bit) in bits.chars().enumerate() {
            if bit != '1' {
                continue;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + col as u32 * scale + dx;
                    let py = y + row as u32 * scale + dy;
                    if px >= image.width() || py >= image.height() {
                        continue;
                    }
                    blend_pixel(image, px, py, Rgba([255, 255, 255, alpha]));
                }
            }
        }
    }
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>) {
    let dst = image.get_pixel_mut(x, y);
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    let blend = |sc: u8, dc: u8| -> u8 {
        (((sc as f32 * sa) + (dc as f32 * da * (1.0 - sa))) / out_a)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    *dst = Rgba([
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]);
}

fn glyph_pattern(ch: char) -> [&'static str; 7] {
    match ch {
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'B' => [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
        'C' => [
            "01111", "10000", "10000", "10000", "10000", "10000", "01111",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'F' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
        'G' => [
            "01111", "10000", "10000", "10011", "10001", "10001", "01111",
        ],
        'H' => [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'I' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
        'J' => [
            "11111", "00010", "00010", "00010", "00010", "10010", "01100",
        ],
        'K' => [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        'N' => [
            "10001", "10001", "11001", "10101", "10011", "10001", "10001",
        ],
        'O' => [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'S' => [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'V' => [
            "10001", "10001", "10001", "10001", "10001", "01010", "00100",
        ],
        'W' => [
            "10001", "10001", "10001", "10101", "10101", "10101", "01010",
        ],
        'X' => [
            "10001", "10001", "01010", "00100", "01010", "10001", "10001",
        ],
        'Y' => [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
        'Z' => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
        '0' => [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        '6' => [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
        '-' => [
            "00000", "00000", "00000", "11111", "00000", "00000", "00000",
        ],
        '_' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "11111",
        ],
        '.' => [
            "00000", "00000", "00000", "00000", "00000", "00110", "00110",
        ],
        ' ' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "00000",
        ],
        _ => [
            "11111", "10001", "00110", "00100", "00110", "10001", "11111",
        ],
    }
}

fn generate_readme_if_configured(cfg: &Config) -> Result<Option<String>> {
    let Some(metadata) = cfg.metadata.as_ref() else {
        return Ok(None);
    };
    let _ = &metadata.itch_template;
    let Some(path) = metadata.readme_template.as_ref() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }

    let tmpl = fs::read_to_string(path)
        .with_context(|| format!("failed reading readme template {}", path.display()))?;
    Ok(Some(render_template(&tmpl, cfg)))
}

fn render_template(template: &str, cfg: &Config) -> String {
    let mut out = template.to_string();
    let license = cfg.pack.license.as_deref().unwrap_or("UNLICENSED");
    let resolutions = cfg
        .build
        .resolutions
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let replacements = [
        ("{{ pack.name }}", cfg.pack.name.as_str()),
        ("{{ pack.slug }}", cfg.pack.slug.as_str()),
        ("{{ pack.author }}", cfg.pack.author.as_str()),
        ("{{ pack.brand }}", cfg.pack.brand.as_str()),
        ("{{ pack.semver }}", cfg.pack.semver.as_str()),
        ("{{ pack.license }}", license),
        (
            "{{ paths.exports }}",
            &normalize_for_glob(&cfg.paths.exports),
        ),
        (
            "{{ paths.previews }}",
            &normalize_for_glob(&cfg.paths.previews),
        ),
    ];

    for (from, to) in replacements {
        out = out.replace(from, to);
    }
    out.replace("{{ build.resolutions | join(\", \") }}", &resolutions)
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-._/:".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
