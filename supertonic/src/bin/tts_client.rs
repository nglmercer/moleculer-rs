//! Supertonic TTS HTTP Client Example
//!
//! This example demonstrates how to call the TTS service via HTTP.
//!
//! # Running
//!
//! First, start the TTS service in one terminal:
//! ```bash
//! cd supertonic
//! cargo run --bin tts-service -- --onnx-dir /path/to/onnx/models --voice-style /path/to/voice_style.json
//! ```
//!
//! Then, run the client in another terminal:
//! ```bash
//! cd supertonic
//! cargo run --bin tts-client
//! ```

use base64;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

/// TTS synthesis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSRequest {
    /// Text to synthesize
    pub text: String,

    /// Language code (en, ko, es, pt, fr)
    pub lang: String,

    /// Speech speed multiplier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,

    /// Number of denoising steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denoising_steps: Option<usize>,
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

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub node_id: String,
}

/// Language information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

/// Language list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageListResponse {
    pub languages: Vec<LanguageInfo>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    println!("🎙️  Supertonic TTS HTTP Client");
    println!("==============================");
    println!();

    let server_url = "http://localhost:8080";
    println!("🎯 Target server: {}", server_url);
    println!();

    // Create HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(60)) // TTS may take a while
        .build()?;

    // Test 1: Health check
    println!("📝 Test 1: Health check");
    match client.get(format!("{}/health", server_url)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let health: HealthResponse = response.json().await?;
                println!("✅ Health check passed:");
                println!("   Status: {}", health.status);
                println!("   Node ID: {}", health.node_id);
            } else {
                println!("❌ Health check failed with status: {}", response.status());
                println!("   Make sure the TTS service is running:");
                println!("   cargo run --bin tts-service -- --onnx-dir /path/to/models");
                return Ok(());
            }
        }
        Err(e) => {
            println!("❌ Failed to connect to server: {}", e);
            println!("   Make sure the TTS service is running:");
            println!("   cargo run --bin tts-service -- --onnx-dir /path/to/models");
            return Ok(());
        }
    }
    println!();

    // Test 2: List languages
    println!("📝 Test 2: List available languages");
    let list_langs_request = serde_json::json!({});

    match client
        .post(format!("{}/publish", server_url))
        .json(&serde_json::json!({
            "topic": "tts.listLanguages",
            "data": serde_json::to_vec(&list_langs_request)?,
            "sender": "tts-client",
            "reply_to": None::<String>
        }))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("✅ Language list request sent");
                println!("   Available languages: en, ko, es, pt, fr");
            } else {
                println!("❌ Language list request failed: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to list languages: {}", e);
        }
    }
    println!();

    // Test 3: Synthesize speech
    println!("📝 Test 3: Synthesize speech");

    let tts_request = TTSRequest {
        text: "Hello, this is a test of the Supertonic TTS service.".to_string(),
        lang: "en".to_string(),
        speed: Some(1.0),
        denoising_steps: Some(4),
    };

    // Create the HTTP message for moleculer
    let message = serde_json::json!({
        "topic": "tts.synthesize",
        "data": serde_json::to_vec(&tts_request)?,
        "sender": "tts-client",
        "reply_to": None::<String>
    });

    println!("   Sending TTS request...");
    match client
        .post(format!("{}/publish", server_url))
        .json(&message)
        .send()
        .await
    {
        Ok(http_resp) => {
            if http_resp.status().is_success() {
                // Get request ID from response
                let resp_json: serde_json::Value = http_resp.json().await?;
                let request_id = resp_json["data"].as_str()
                    .ok_or_else(|| eyre::eyre!("No request ID in response"))?;
                
                println!("   Request ID: {}", request_id);
                println!("   Waiting for TTS response...");
                
                // Poll for response file
                let response_file = Path::new("output/responses").join(format!("{}.json", request_id));
                let max_attempts = 30;
                let mut attempts = 0;
                let mut response_data = None;
                
                while attempts < max_attempts {
                    if response_file.exists() {
                        match std::fs::read_to_string(&response_file) {
                            Ok(content) => {
                                response_data = Some(serde_json::from_str(&content)?);
                                break;
                            }
                            Err(_) => {}
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    attempts += 1;
                }
                
                if let Some(response) = response_data {
                    if response["success"].as_bool().unwrap_or(false) {
                        if let Some(audio_base64) = response["audio"].as_str() {
                            let audio_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, audio_base64)?;
                            
                            // Create output directory if it doesn't exist
                            let output_dir = Path::new("output");
                            if !output_dir.exists() {
                                std::fs::create_dir_all(output_dir)?;
                            }
                            
                            // Write audio to file
                            let output_path = output_dir.join("audio.wav");
                            let mut file = File::create(&output_path)?;
                            file.write_all(&audio_bytes)?;
                            
                            println!("✅ Audio saved to: {:?}", output_path);
                            println!("   Duration: {:.2}s", response["duration"].as_f64().unwrap_or(0.0));
                            println!("   Sample rate: {} Hz", response["sample_rate"].as_i64().unwrap_or(0));
                        } else {
                            println!("❌ No audio data in response");
                        }
                    } else {
                        println!("❌ TTS synthesis failed: {:?}", response["error"]);
                    }
                } else {
                    println!("❌ Timeout waiting for TTS response");
                }
            } else {
                println!("❌ TTS request failed with status: {}", http_resp.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to synthesize speech: {}", e);
        }
    }
    println!();

    // Test 4: Synthesize in different language
    println!("📝 Test 4: Synthesize in Spanish");

    let tts_request_es = TTSRequest {
        text: "Hola, esta es una prueba del servicio TTS.".to_string(),
        lang: "es".to_string(),
        speed: Some(1.0),
        denoising_steps: Some(4),
    };

    println!("   Text: \"{}\"", tts_request_es.text);
    println!("   Language: {}", tts_request_es.lang);

    // Use direct synthesize endpoint
    match client
        .post(format!("{}/synthesize", server_url))
        .json(&tts_request_es)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                // Parse the response and save audio
                let response: TTSResponse = response.json().await?;
                if response.success {
                    if let Some(audio_base64) = response.audio {
                        let audio_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &audio_base64)?;
                        
                        let output_dir = Path::new("output");
                        if !output_dir.exists() {
                            std::fs::create_dir_all(output_dir)?;
                        }
                        
                        let output_path = output_dir.join("audio_es.wav");
                        let mut file = File::create(&output_path)?;
                        file.write_all(&audio_bytes)?;
                        
                        println!("✅ Spanish audio saved to: {:?}", output_path);
                    } else {
                        println!("❌ No audio data in Spanish response");
                    }
                } else {
                    println!("❌ Spanish TTS synthesis failed: {:?}", response.error);
                }
            } else {
                println!("❌ Spanish TTS request failed: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Failed to synthesize Spanish speech: {}", e);
        }
    }
    println!();

    println!("🎉 All tests completed!");
    println!();
    println!("💡 Note: This is a demonstration client.");
    println!("   For actual TTS functionality, ensure ONNX models are available.");

    Ok(())
}
