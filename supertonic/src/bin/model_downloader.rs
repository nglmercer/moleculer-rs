//! Model Downloader for Supertonic TTS
//!
//! This utility downloads ONNX models from Hugging Face Hub and caches them locally.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin model-downloader -- --model Supertone/supertonic --cache ./cache
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Model downloader arguments
#[derive(Parser, Debug)]
#[command(name = "model-downloader")]
#[command(about = "Download ONNX models for Supertonic TTS")]
struct Args {
    /// Hugging Face model repository ID
    #[arg(short, long, default_value = "Supertone/supertonic")]
    model: String,

    /// Local cache directory for models
    #[arg(short, long, default_value = "./cache")]
    cache: PathBuf,

    /// Force re-download even if files exist
    #[arg(short, long)]
    force: bool,

    /// List of specific files to download (optional)
    #[arg(long)]
    files: Option<Vec<String>>,
}

/// Required model files for Supertonic TTS
const REQUIRED_FILES: &[&str] = &[
    "tokenizer_config.json",
    "config.json",
    "tokenizer.json",
    "voice_decoder.onnx",
    "latent_denoiser.onnx",
    "text_encoder.onnx",
];

/// Hugging Face Hub API URL
const HF_API_URL: &str = "https://huggingface.co";

/// Download a file from URL to path
fn download_file(url: &str, dest: &Path) -> Result<()> {
    info!("Downloading: {}", url);

    let response = ureq::get(url)
        .call()
        .context("Failed to make HTTP request")?;

    if response.status() != 200 {
        anyhow::bail!("HTTP error: {}", response.status());
    }

    let mut file = File::create(dest).context("Failed to create destination file")?;

    let mut reader = response.into_reader();
    io::copy(&mut reader, &mut file).context("Failed to write file")?;

    Ok(())
}

/// Check if a file exists and get its size
fn file_info(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.len())
}

/// Download model files from Hugging Face
fn download_model_files(
    model_id: &str,
    cache_dir: &Path,
    files: Option<&[String]>,
    force: bool,
) -> Result<()> {
    // Create cache directory
    fs::create_dir_all(cache_dir).context("Failed to create cache directory")?;

    info!("Cache directory: {}", cache_dir.display());
    info!("Model ID: {}", model_id);

    // Determine which files to download
    let files_to_download: Vec<&str> = match files {
        Some(f) => f.iter().map(|s| s.as_str()).collect(),
        None => REQUIRED_FILES.to_vec(),
    };

    let base_url = format!("{}/{}", HF_API_URL, model_id);

    let mut downloaded = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for file_name in files_to_download {
        let dest_path = cache_dir.join(file_name);

        // Check if file already exists
        if !force {
            if let Some(size) = file_info(&dest_path) {
                if size > 0 {
                    info!("✓ Already cached: {} ({} bytes)", file_name, size);
                    skipped += 1;
                    continue;
                }
            }
        }

        // Try to download from Hugging Face
        let url = format!("{}/resolve/main/onnx/{}", base_url, file_name);

        match download_file(&url, &dest_path) {
            Ok(_) => {
                info!("✓ Downloaded: {}", file_name);
                downloaded += 1;
            }
            Err(e) => {
                warn!("✗ Failed to download {}: {}", file_name, e);
                failed += 1;
            }
        }
    }

    info!("Download summary:");
    info!("  Downloaded: {}", downloaded);
    info!("  Skipped (cached): {}", skipped);
    info!("  Failed: {}", failed);

    if failed > 0 {
        warn!("Some files failed to download. The model may not be available on Hugging Face.");
        warn!("You may need to manually download the ONNX models.");
    }

    Ok(())
}

/// Create a sample config file if none exists
fn create_sample_config(cache_dir: &Path) -> Result<()> {
    let config_path = cache_dir.join("tts.json");

    if config_path.exists() {
        info!("Config file already exists");
        return Ok(());
    }

    info!("Creating sample config file...");

    let sample_config = serde_json::json!({
        "ae": {
            "sample_rate": 22050,
            "base_chunk_size": 2048
        },
        "ttl": {
            "chunk_compress_factor": 1,
            "latent_dim": 16
        }
    });

    let file = File::create(&config_path).context("Failed to create config file")?;
    serde_json::to_writer_pretty(file, &sample_config).context("Failed to write config JSON")?;

    info!("Created sample config at: {}", config_path.display());

    Ok(())
}

/// Verify downloaded files
fn verify_files(cache_dir: &Path) -> Result<()> {
    info!("Verifying cached files...");

    let mut all_present = true;

    for file_name in REQUIRED_FILES {
        let path = cache_dir.join(file_name);
        if path.exists() {
            let size = fs::metadata(&path)?.len();
            info!("✓ {}: {} bytes", file_name, size);
        } else {
            warn!("✗ {}: MISSING", file_name);
            all_present = false;
        }
    }

    if all_present {
        info!("✅ All required files are present!");
        info!("You can now run the TTS service:");
        info!("  cargo run --bin tts-service -- --onnx-dir ./cache");
    } else {
        warn!("⚠️  Some files are missing. Please download them manually.");
    }

    Ok(())
}

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    info!("🚀 Supertonic TTS Model Downloader");
    info!("==================================");

    // Download model files
    download_model_files(&args.model, &args.cache, args.files.as_deref(), args.force)?;

    // Create sample files if needed
    create_sample_config(&args.cache)?;

    // Verify files
    verify_files(&args.cache)?;

    Ok(())
}
