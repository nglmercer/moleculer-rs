//! Direct TTS Engine Test
//!
//! This binary tests the TTS engine directly without HTTP, to validate
//! that the ONNX models are working correctly.

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod helpers {
    include!("../../helpers.rs");
}
use helpers::{load_text_to_speech, load_voice_style};

#[derive(Parser, Debug)]
#[command(name = "direct-test")]
#[command(about = "Direct TTS engine test without HTTP")]
#[command(version)]
struct Args {
    /// Directory containing ONNX models
    #[arg(short, long, default_value = "./onnx_models")]
    onnx_dir: PathBuf,

    /// Path to voice style JSON file
    #[arg(short, long, default_value = "./voice_style.json")]
    voice_style: PathBuf,

    /// Text to synthesize
    #[arg(short, long, default_value = "Hello, this is a direct test of the TTS engine.")]
    text: String,

    /// Language code (en, ko, es, pt, fr)
    #[arg(short, long, default_value = "en")]
    lang: String,

    /// Output WAV file path
    #[arg(long, default_value = "./direct_test_output.wav")]
    output: PathBuf,

    /// Number of denoising steps
    #[arg(long, default_value = "4")]
    denoising_steps: usize,

    /// Speech speed multiplier (1.0 = normal)
    #[arg(long, default_value = "1.0")]
    speed: f32,

    /// Silence duration between chunks in seconds
    #[arg(long, default_value = "0.2")]
    silence_duration: f32,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse arguments
    let args = Args::parse();

    // Initialize logging
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .without_time()
        .init();

    info!("🎯 Direct TTS Engine Test");
    info!("==========================");
    info!("ONNX Directory: {:?}", args.onnx_dir);
    info!("Voice Style: {:?}", args.voice_style);
    info!("Text: \"{}\"", args.text);
    info!("Language: {}", args.lang);
    info!("Output: {:?}", args.output);

    // Load TTS engine
    info!("Loading TTS engine...");
    let mut tts = load_text_to_speech(args.onnx_dir.to_str().unwrap(), false)
        .context("Failed to load TTS engine")?;
    info!("TTS engine loaded successfully");

    // Load voice style
    info!("Loading voice style...");
    let style = load_voice_style(&[args.voice_style.to_str().unwrap().to_string()], true)
        .context("Failed to load voice style")?;
    info!("Voice style loaded successfully");

    // Perform synthesis
    info!("Starting synthesis...");
    let (audio_data, duration) = tts.call(
        &args.text,
        &args.lang,
        &style,
        args.denoising_steps,
        args.speed,
        args.silence_duration,
    )
    .context("Synthesis failed")?;

    info!("Synthesis complete: duration={:.2}s, samples={}", duration, audio_data.len());

    // Write WAV file
    std::fs::create_dir_all(args.output.parent().unwrap_or_else(|| std::path::Path::new(".")))
        .context("Failed to create output directory")?;
    helpers::write_wav_file(&args.output, &audio_data, tts.sample_rate)
        .context("Failed to write WAV file")?;
    info!("WAV file written: {:?}", args.output);

    Ok(())
}
