//! Supertonic TTS Service with HTTP Protocol
//!
//! This example demonstrates how to create a Text-to-Speech microservice
//! using moleculer-rs with HTTP transport. The service exposes TTS actions
//! that can be called via HTTP.
//!
//! # Features
//!
//! - HTTP-based TTS service using moleculer-rs
//! - Supports multiple languages (en, ko, es, pt, fr)
//! - Returns base64-encoded WAV audio
//! - Configurable voice style, speed, and denoising steps
//!
//! # Running
//!
//! ```bash
//! cd supertonic
//! cargo run --bin tts-service -- --onnx-dir /path/to/onnx/models --voice-style /path/to/voice_style.json
//! ```
//!
//! # Endpoints
//!
//! Once running, the service provides:
//! - GET  /health - Health check
//! - GET  /info   - Node information
//! - POST /publish - Publish message
//! - POST /request - Request/reply
//!
//! # Testing with curl
//!
//! ```bash
//! # Health check
//! curl http://localhost:8080/health
//!
//! # Call TTS action via publish endpoint
//! curl -X POST http://localhost:8080/publish \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "topic": "tts.synthesize",
//!     "data": [123,34,116,101,120,116,34,58,34,72,101,108,108,111,32,119,111,114,108,100,34,44,34,108,97,110,103,34,58,34,101,110,34,125],
//!     "sender": "curl",
//!     "reply_to": null
//!   }'
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use moleculer::{
    config::{ConfigBuilder, Transporter},
    service::{ActionBuilder, Service},
    ActionContext, ServiceBroker,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;
use warp::Filter;

// Import the helpers module from the parent
mod helpers {
    include!("../../helpers.rs");
}

use helpers::{is_valid_lang, load_text_to_speech, load_voice_style, Style, TextToSpeech};
use ndarray::Array3;

/// Global TTS engine instance
static TTS_ENGINE: OnceCell<Mutex<TextToSpeech>> = OnceCell::new();

/// Global voice style instance
static VOICE_STYLE: OnceCell<Style> = OnceCell::new();

/// Global write WAV flag
static WRITE_WAV: OnceCell<std::sync::atomic::AtomicBool> = OnceCell::new();

/// Global output directory
static OUTPUT_DIR: OnceCell<PathBuf> = OnceCell::new();

/// Global node ID for HTTP responses
static NODE_ID: OnceCell<String> = OnceCell::new();

/// Default ONNX models directory
const DEFAULT_ONNX_DIR: &str = "./onnx_models";

/// Default voice style path
const DEFAULT_VOICE_STYLE: &str = "./voice_style.json";

// ============================================================================
// ============================================================================
// Direct HTTP Server for /synthesize endpoint
// ============================================================================


fn default_speed_val() -> f32 {
    1.0
}

fn default_denoising_steps_val() -> usize {
    4
}

fn default_silence_duration_val() -> f32 {
    0.2
}
/// Direct TTS synthesis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectTTSRequest {
    pub text: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_speed_val")]
    pub speed: f32,
    #[serde(default = "default_denoising_steps_val")]
    pub denoising_steps: usize,
    #[serde(default = "default_silence_duration_val")]
    pub silence_duration: f32,
}

/// Direct TTS response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectTTSResponse {
    pub success: bool,
    pub audio: Option<String>,
    pub duration: Option<f32>,
    pub sample_rate: Option<i32>,
    pub error: Option<String>,
}

/// Start direct HTTP server for /synthesize endpoint
async fn start_direct_http_server(address: &str) -> Result<JoinHandle<()>> {
    let node_id = NODE_ID.get()
        .map(|s| s.clone())
        .unwrap_or_else(|| "tts-service".to_string());
    
    // Health check endpoint
    let health = warp::path!("health").map(move || {
        warp::reply::json(&serde_json::json!({
            "status": "ok",
            "node_id": node_id.clone(),
        }))
    });
    
    // Synthesize endpoint - direct TTS synthesis
    let synthesize = warp::path!("synthesize")
        .and(warp::post())
        .and(warp::body::json())
        .then(|request: DirectTTSRequest| async move {
            match synthesize_direct(request).await {
                Ok(response) => warp::reply::json(&response),
                Err(e) => warp::reply::json(&DirectTTSResponse {
                    success: false,
                    audio: None,
                    duration: None,
                    sample_rate: None,
                    error: Some(e.to_string()),
                }),
            }
        });
    
    let routes = health.or(synthesize);
    
    let addr: SocketAddr = address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?;

    let server = warp::serve(routes).bind(addr).await;

    let handle = tokio::spawn(async move {
        server.run().await;
    });

    Ok(handle)
}
/// Perform direct TTS synthesis
async fn synthesize_direct(request: DirectTTSRequest) -> Result<DirectTTSResponse> {
    info!("Direct synthesis: text='{}', lang={}", request.text, request.lang);
    
    if !is_valid_lang(&request.lang) {
        return Ok(DirectTTSResponse {
            success: false,
            audio: None,
            duration: None,
            sample_rate: None,
            error: Some(format!(
                "Invalid language: {}. Available: {:?}",
                request.lang,
                helpers::AVAILABLE_LANGS
            )),
        });
    }
    
    let tts_engine = TTS_ENGINE.get()
        .ok_or_else(|| anyhow::anyhow!("TTS engine not initialized"))?;
    
    let voice_style = VOICE_STYLE.get()
        .ok_or_else(|| anyhow::anyhow!("Voice style not initialized"))?;
    
    let (audio_data, duration) = {
        let mut engine = tts_engine.lock().map_err(|e| anyhow::anyhow!("Mutex lock failed: {}", e))?;
        engine.call(
            &request.text,
            &request.lang,
            voice_style,
            request.denoising_steps,
            request.speed,
            request.silence_duration,
        )?
    };
    
    let sample_rate = helpers::load_cfgs(DEFAULT_ONNX_DIR)?.ae.sample_rate;
    
    // Write WAV file if enabled
    if WRITE_WAV.get()
        .map(|b| b.load(Ordering::Relaxed))
        .unwrap_or(false)
    {
        if let Some(output_dir) = OUTPUT_DIR.get() {
            let text_preview = request.text
                .chars()
                .take(30)
                .collect::<String>()
                .replace(|c: char| !c.is_alphanumeric(), "_");
            
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let filename = format!(
                "{}/{}_{}_{}.wav",
                output_dir.display(),
                request.lang,
                text_preview,
                timestamp
            );
            
            if let Err(e) = helpers::write_wav_file(&filename, &audio_data, sample_rate) {
                warn!("Failed to write WAV file: {}", e);
            } else {
                info!("WAV file written: {}", filename);
            }
        }
    }
    
    let wav_bytes = {
        let mut wav_data: Vec<u8> = Vec::new();
        write_wav_to_memory(&audio_data, sample_rate, &mut wav_data)?;
        wav_data
    };
    
    let audio_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &wav_bytes);
    
    info!("Direct synthesis complete: duration={:.2}s, size={} bytes", duration, wav_bytes.len());
    
    Ok(DirectTTSResponse {
        success: true,
        audio: Some(audio_base64),
        duration: Some(duration),
        sample_rate: Some(sample_rate),
        error: None,
    })
}

// CLI Arguments
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "tts-service")]
#[command(about = "Supertonic TTS Microservice with HTTP Protocol")]
#[command(version)]
struct Args {
    /// Directory containing ONNX models
    #[arg(short, long, default_value = DEFAULT_ONNX_DIR)]
    onnx_dir: PathBuf,

    /// Path to voice style JSON file
    #[arg(short, long, default_value = DEFAULT_VOICE_STYLE)]
    voice_style: PathBuf,

    /// HTTP server address (host:port)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    address: String,

    /// Node ID for the service
    #[arg(short, long, default_value = "tts-service-node")]
    node_id: String,

    /// Number of denoising steps (higher = better quality, slower)
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

    /// Directory to write output WAV files
    #[arg(long, default_value = "./output")]
    output_dir: PathBuf,

    /// Enable writing WAV files (for testing)
    #[arg(long, default_value = "false")]
    write_wav: bool,
}

// ============================================================================
// Request/Response Structures
// ============================================================================

/// TTS synthesis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSRequest {
    /// Text to synthesize
    pub text: String,

    /// Language code (en, ko, es, pt, fr)
    #[serde(default = "default_lang")]
    pub lang: String,

    /// Speech speed multiplier
    #[serde(default = "default_speed")]
    pub speed: Option<f32>,

    /// Number of denoising steps
    #[serde(default = "default_denoising_steps")]
    pub denoising_steps: Option<usize>,

    /// Silence duration between chunks
    #[serde(default = "default_silence_duration")]
    pub silence_duration: Option<f32>,
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_speed() -> Option<f32> {
    Some(1.0)
}

fn default_denoising_steps() -> Option<usize> {
    Some(4)
}

fn default_silence_duration() -> Option<f32> {

    Some(0.2)
}

/// TTS synthesis response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSResponse {
    /// Whether synthesis was successful
    pub success: bool,

    /// Base64-encoded WAV audio data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,

    /// Audio duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,

    /// Sample rate of the audio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,

    /// Error message if synthesis failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Language list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageListRequest {}

/// Language list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageListResponse {
    /// Available language codes
    pub languages: Vec<LanguageInfo>,
}

/// Language information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    /// Language code
    pub code: String,

    /// Language name
    pub name: String,
}

// ============================================================================
// TTS Action Handlers
// ============================================================================

/// Handle TTS synthesis request
fn handle_tts_synthesize(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
    info!("Received TTS synthesis request");

    // Parse request
    let request: TTSRequest = match serde_json::from_value(ctx.params.clone()) {
        Ok(req) => req,
        Err(e) => {
            let error_response = TTSResponse {
                success: false,
                audio: None,
                duration: None,
                sample_rate: None,
                error: Some(format!("Invalid request: {}", e)),
            };
            ctx.reply(serde_json::to_value(error_response)?);
            return Ok(());
        }
    };

    info!(
        "Synthesizing text: '{}' (lang: {}, speed: {})",
        request.text,
        request.lang,
        request.speed.unwrap_or(1.0)
    );

    // Validate language
    if !is_valid_lang(&request.lang) {
        let error_response = TTSResponse {
            success: false,
            audio: None,
            duration: None,
            sample_rate: None,
            error: Some(format!(
                "Invalid language: {}. Available: {:?}",
                request.lang,
                helpers::AVAILABLE_LANGS
            )),
        };
        ctx.reply(serde_json::to_value(error_response)?);
        return Ok(());
    }

    // Get TTS engine and voice style
    let tts_engine = match TTS_ENGINE.get() {
        Some(engine) => engine,
        None => {
            let error_response = TTSResponse {
                success: false,
                audio: None,
                duration: None,
                sample_rate: None,
                error: Some("TTS engine not initialized".to_string()),
            };
            ctx.reply(serde_json::to_value(error_response)?);
            return Ok(());
        }
    };

    let voice_style = match VOICE_STYLE.get() {
        Some(style) => style,
        None => {
            let error_response = TTSResponse {
                success: false,
                audio: None,
                duration: None,
                sample_rate: None,
                error: Some("Voice style not initialized".to_string()),
            };
            ctx.reply(serde_json::to_value(error_response)?);
            return Ok(());
        }
    };

    // Perform TTS synthesis
    let result = {
        let mut engine = tts_engine
            .lock()
            .map_err(|e| format!("Failed to lock TTS engine: {}", e))?;

        engine.call(
            &request.text,
            &request.lang,
            voice_style,
            request.denoising_steps.unwrap_or(4),
            request.speed.unwrap_or(1.0),
            request.silence_duration.unwrap_or(0.2),
        )
    };

    match result {
        Ok((audio_data, duration)) => {
            // Get sample rate
            let sample_rate = helpers::load_cfgs(DEFAULT_ONNX_DIR).unwrap().ae.sample_rate;

            // Write WAV file if enabled
            if WRITE_WAV
                .get()
                .map(|b| b.load(Ordering::Relaxed))
                .unwrap_or(false)
            {
                if let Some(output_dir) = OUTPUT_DIR.get() {
                    // Create filename from text (first 30 chars)
                    let text_preview = request
                        .text
                        .chars()
                        .take(30)
                        .collect::<String>()
                        .replace(|c: char| !c.is_alphanumeric(), "_");

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let filename = format!(
                        "{}/{}_{}_{}.wav",
                        output_dir.display(),
                        request.lang,
                        text_preview,
                        timestamp
                    );

                    match helpers::write_wav_file(&filename, &audio_data, sample_rate) {
                        Ok(_) => info!("WAV file written: {}", filename),
                        Err(e) => warn!("Failed to write WAV file: {}", e),
                    }
                }
            }

            // Convert audio to WAV bytes
            let mut wav_bytes: Vec<u8> = Vec::new();
            {
                // Create WAV in memory
                if let Err(e) = write_wav_to_memory(&audio_data, sample_rate, &mut wav_bytes) {
                    let error_response = TTSResponse {
                        success: false,
                        audio: None,
                        duration: None,
                        sample_rate: None,
                        error: Some(format!("Failed to create WAV: {}", e)),
                    };
                    ctx.reply(serde_json::to_value(error_response)?);
                    return Ok(());
                }
            }

            // Encode to base64
            let audio_base64 =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &wav_bytes);

            // Generate request ID and write response file
            let request_id = Uuid::new_v4().to_string();

            // Create response JSON
            let response_json = serde_json::json!({
                "success": true,
                "audio": audio_base64,
                "duration": duration,
                "sample_rate": sample_rate,
                "request_id": request_id,
            });

            // Write response to file
            let response_dir = OUTPUT_DIR
                .get()
                .map(|p| p.join("responses"))
                .unwrap_or_else(|| {
                    std::path::Path::new("output")
                        .join("responses")
                        .to_path_buf()
                });
            let _ = std::fs::create_dir_all(&response_dir);
            let response_file = response_dir.join(format!("{}.json", request_id));
            let _ = std::fs::write(&response_file, serde_json::to_string(&response_json)?);

            // Return response to caller (via moleculer reply)
            let response = TTSResponse {
                success: true,
                audio: Some(audio_base64),
                duration: Some(duration),
                sample_rate: Some(sample_rate),
                error: None,
            };

            info!(
                "Synthesis complete: duration={:.2}s, request_id={}",
                duration, request_id
            );
            ctx.reply(serde_json::to_value(response)?);
        }
        Err(e) => {
            // Generate request ID and write error response file
            let request_id = Uuid::new_v4().to_string();

            let error_response_json = serde_json::json!({
                "success": false,
                "error": format!("Synthesis failed: {}", e),
                "request_id": request_id,
            });

            // Write error response to file
            let response_dir = OUTPUT_DIR
                .get()
                .map(|p| p.join("responses"))
                .unwrap_or_else(|| {
                    std::path::Path::new("output")
                        .join("responses")
                        .to_path_buf()
                });
            let _ = std::fs::create_dir_all(&response_dir);
            let response_file = response_dir.join(format!("{}.json", request_id));
            let _ = std::fs::write(&response_file, serde_json::to_string(&error_response_json)?);

            let error_response = TTSResponse {
                success: false,
                audio: None,
                duration: None,
                sample_rate: None,
                error: Some(format!("Synthesis failed: {}", e)),
            };
            ctx.reply(serde_json::to_value(error_response)?);
        }
    }

    Ok(())
}

/// Handle language list request
fn handle_list_languages(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
    info!("Received language list request");

    let languages = vec![
        LanguageInfo {
            code: "en".to_string(),
            name: "English".to_string(),
        },
        LanguageInfo {
            code: "ko".to_string(),
            name: "Korean".to_string(),
        },
        LanguageInfo {
            code: "es".to_string(),
            name: "Spanish".to_string(),
        },
        LanguageInfo {
            code: "pt".to_string(),
            name: "Portuguese".to_string(),
        },
        LanguageInfo {
            code: "fr".to_string(),
            name: "French".to_string(),
        },
    ];

    let response = LanguageListResponse { languages };
    ctx.reply(serde_json::to_value(response)?);

    Ok(())
}

/// Handle health check request
fn handle_health(ctx: ActionContext) -> Result<(), Box<dyn Error>> {
    let health = serde_json::json!({
        "status": "healthy",
        "tts_engine_loaded": TTS_ENGINE.get().is_some(),
        "voice_style_loaded": VOICE_STYLE.get().is_some(),
    });
    ctx.reply(health);
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Write WAV data to a byte vector
fn write_wav_to_memory(audio_data: &[f32], sample_rate: i32, output: &mut Vec<u8>) -> Result<()> {
    use byteorder::{LittleEndian, WriteBytesExt};
    use std::io::Write;

    let num_samples = audio_data.len() as u32;
    let bytes_per_sample = 2u16;
    let num_channels = 1u16;
    let byte_rate = (sample_rate as u32) * (num_channels as u32) * (bytes_per_sample as u32);
    let block_align = num_channels * bytes_per_sample;
    let data_size = num_samples * (bytes_per_sample as u32);
    let file_size = 36 + data_size;

    // RIFF header
    output.write_all(b"RIFF")?;
    output.write_u32::<LittleEndian>(file_size)?;
    output.write_all(b"WAVE")?;

    // fmt chunk
    output.write_all(b"fmt ")?;
    output.write_u32::<LittleEndian>(16)?; // chunk size
    output.write_u16::<LittleEndian>(1)?; // audio format (PCM)
    output.write_u16::<LittleEndian>(num_channels)?;
    output.write_u32::<LittleEndian>(sample_rate as u32)?;
    output.write_u32::<LittleEndian>(byte_rate)?;
    output.write_u16::<LittleEndian>(block_align)?;
    output.write_u16::<LittleEndian>(bytes_per_sample * 8)?; // bits per sample

    // data chunk
    output.write_all(b"data")?;
    output.write_u32::<LittleEndian>(data_size)?;

    // Write audio samples
    for &sample in audio_data {
        let clamped = sample.clamp(-1.0, 1.0);
        let val = (clamped * 32767.0) as i16;
        output.write_i16::<LittleEndian>(val)?;
    }

    Ok(())
}

/// Initialize the TTS engine
fn init_tts_engine(onnx_dir: &PathBuf) -> Result<TextToSpeech> {
    info!("Loading TTS engine from: {:?}", onnx_dir);

    let tts = load_text_to_speech(onnx_dir.to_str().unwrap(), false)
        .context("Failed to load TTS engine")?;

    info!("TTS engine loaded successfully");
    Ok(tts)
}

/// Initialize the voice style
fn init_voice_style(voice_style_path: &PathBuf) -> Result<Style> {
    info!("Loading voice style from: {:?}", voice_style_path);

    let style = load_voice_style(&[voice_style_path.to_str().unwrap().to_string()], true)
        .context("Failed to load voice style")?;

    info!("Voice style loaded successfully");
    Ok(style)
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Parse command line arguments
    let args = Args::try_parse().unwrap_or_else(|e| {
        eprintln!("Error parsing arguments: {}", e);
        std::process::exit(1);
    });

    
    // Set global node ID
    let _ = NODE_ID.set(args.node_id.clone());
    
    // Start direct HTTP server for /synthesize endpoint
    let _http_server_handle = tokio::spawn(async move {
        if let Err(e) = start_direct_http_server(&format!("0.0.0.0:{}", 8081)).await {
            warn!("Failed to start direct HTTP server: {}", e);
        }
    });
    
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

    info!("🚀 Starting Supertonic TTS Service");
    info!("==================================");
    info!("Node ID: {}", args.node_id);
    info!("Address: {}", args.address);
    info!("ONNX Directory: {:?}", args.onnx_dir);
    info!("Voice Style: {:?}", args.voice_style);

    // Initialize TTS engine
    let tts_engine = match init_tts_engine(&args.onnx_dir) {
        Ok(engine) => engine,
        Err(e) => {
            log::error!("Failed to initialize TTS engine: {:#}", e);
            log::warn!("Service will start but TTS actions will return errors");
            // Create a dummy engine for demonstration
            // In production, you would exit here
            return Err(eyre::eyre!("Failed to initialize TTS engine: {:#}", e));
        }
    };

    // Store TTS engine globally
    TTS_ENGINE
        .set(Mutex::new(tts_engine))
        .map_err(|_| eyre::eyre!("Failed to set TTS engine"))?;

    // Initialize voice style
    let voice_style = match init_voice_style(&args.voice_style) {
        Ok(style) => style,
        Err(e) => {
            log::error!("Failed to load voice style: {:#}", e);
            // Create a default style with zeros
            log::warn!("Using default voice style (zeros)");
            Style {
                ttl: Array3::zeros((1, 256, 1)),
                dp: Array3::zeros((1, 256, 1)),
            }
        }
    };

    // Store voice style globally
    VOICE_STYLE
        .set(voice_style)
        .map_err(|_| eyre::eyre!("Failed to set voice style"))?;

    // Initialize write WAV flag
    WRITE_WAV
        .set(AtomicBool::new(args.write_wav))
        .map_err(|_| eyre::eyre!("Failed to set write WAV flag"))?;

    // Initialize output directory
    std::fs::create_dir_all(&args.output_dir).map_err(|e| {
        eyre::eyre!(
            "Failed to create output directory {:?}: {}",
            args.output_dir,
            e
        )
    })?;
    OUTPUT_DIR
        .set(args.output_dir.clone())
        .map_err(|_| eyre::eyre!("Failed to set output directory"))?;

    info!(
        "Output directory: {:?} (WAV writing: {})",
        args.output_dir, args.write_wav
    );

    // Build moleculer config with HTTP transporter
    let config = ConfigBuilder::default()
        .node_id(&args.node_id)
        .transporter(Transporter::http(&args.address))
        .log_level(log::Level::Info)
        .build();

    info!("📡 HTTP Transport configured on {}", args.address);
    info!("   Endpoints:");
    info!("   - GET  /health - Health check");
    info!("   - GET  /info   - Node information");
    info!("   - POST /publish - Publish message (for actions)");
    info!("   - POST /request - Request/reply");

    // Create TTS service with actions
    let synthesize_action = ActionBuilder::new("tts.synthesize")
        .add_callback(handle_tts_synthesize)
        .build();

    let list_languages_action = ActionBuilder::new("tts.listLanguages")
        .add_callback(handle_list_languages)
        .build();

    let health_action = ActionBuilder::new("tts.health")
        .add_callback(handle_health)
        .build();

    let tts_service = Service::new("tts")
        .add_action(synthesize_action)
        .add_action(list_languages_action)
        .add_action(health_action);

    info!("📦 Service 'tts' registered with actions:");
    info!("   - tts.synthesize (synthesize text to speech)");
    info!("   - tts.listLanguages (list available languages)");
    info!("   - tts.health (health check)");

    // Create service broker
    let service_broker = ServiceBroker::new(config).add_service(tts_service);

    info!("✅ TTS Service ready! Press Ctrl+C to stop.");
    info!("💡 Test with curl:");
    info!(
        "   curl http://localhost:{}/health",
        args.address.split(':').nth(1).unwrap_or("8080")
    );

    // Start the service broker (this will run forever)
    service_broker.start().await;

    Ok(())
}
