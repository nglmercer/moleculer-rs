# Supertonic TTS Microservice

A Text-to-Speech microservice built with [moleculer-rs](https://github.com/primcloud/moleculer-rs) using HTTP transport.

## Features

- 🚀 HTTP-based TTS service using moleculer-rs
- 🌍 Supports multiple languages (en, ko, es, pt, fr)
- 📦 Returns base64-encoded WAV audio
- ⚙️ Configurable voice style, speed, and denoising steps
- 📥 Automatic model downloading from Hugging Face Hub

## Prerequisites

- Rust 1.70+
- ONNX Runtime (optional, for GPU support)

## Quick Start

### 1. Download Models

The models are downloaded from [Supertone/supertonic](https://huggingface.co/Supertone/supertonic) repository.

```bash
cd supertonic
cargo run --bin model-downloader -- --cache ./cache
```

Or use the demo script:

```bash
./demo.sh download
```

### 2. Start the TTS Service

```bash
cargo run --bin tts-service -- \
    --onnx-dir ./cache \
    --address 0.0.0.0:8080
```

Or use the demo script:

```bash
./demo.sh service
```

### 3. Test the Service

```bash
# Health check
curl http://localhost:8080/health

# Node info
curl http://localhost:8080/info

# Run test client
cargo run --bin tts-client
```

## HTTP Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/info` | GET | Node information |
| `/publish` | POST | Publish message (for actions) |
| `/request` | POST | Request/reply pattern |

## API

### TTS Synthesize

**Topic:** `tts.synthesize`

**Request:**
```json
{
  "text": "Hello, world!",
  "lang": "en",
  "speed": 1.0,
  "denoising_steps": 4,
  "silence_duration": 0.2
}
```

**Response:**
```json
{
  "success": true,
  "audio": "base64-encoded-wav-audio",
  "duration": 2.5,
  "sample_rate": 22050
}
```

### List Languages

**Topic:** `tts.listLanguages`

**Response:**
```json
{
  "languages": [
    {"code": "en", "name": "English"},
    {"code": "ko", "name": "Korean"},
    {"code": "es", "name": "Spanish"},
    {"code": "pt", "name": "Portuguese"},
    {"code": "fr", "name": "French"}
  ]
}
```

### Health Check

**Topic:** `tts.health`

**Response:**
```json
{
  "status": "healthy",
  "tts_engine_loaded": true,
  "voice_style_loaded": true
}
```

## Command Line Options

### tts-service

```
Options:
      --onnx-dir <ONNX_DIR>          Directory containing ONNX models [default: ./onnx_models]
      --voice-style <VOICE_STYLE>    Path to voice style JSON file [default: ./voice_style.json]
      --address <ADDRESS>            HTTP server address (host:port) [default: 0.0.0.0:8080]
      --node-id <NODE_ID>            Node ID for the service [default: tts-service-node]
      --denoising-steps <STEPS>      Number of denoising steps [default: 4]
      --speed <SPEED>                Speech speed multiplier [default: 1.0]
      --silence-duration <DURATION>  Silence duration between chunks [default: 0.2]
      --log-level <LEVEL>            Log level (trace, debug, info, warn, error) [default: info]
```

### model-downloader

```
Options:
  -m, --model <MODEL>      Hugging Face model repository ID [default: Supertone/supertonic]
  -c, --cache <CACHE>      Local cache directory for models [default: ./cache]
  -f, --force              Force re-download even if files exist
      --files <FILES>      List of specific files to download
```

## Project Structure

```
supertonic/
├── Cargo.toml              # Project dependencies
├── helpers.rs              # TTS helper functions
├── demo.sh                 # Demo script
├── README.md               # This file
├── cache/                  # Downloaded models (created by model-downloader)
│   ├── tokenizer_config.json
│   ├── config.json
│   ├── tokenizer.json
│   ├── voice_decoder.onnx
│   ├── latent_denoiser.onnx
│   └── text_encoder.onnx
└── src/
    └── bin/
        ├── tts_service.rs      # TTS microservice
        ├── tts_client.rs       # Test client
        └── model_downloader.rs # Model download utility
```

## Demo Script

The `demo.sh` script provides convenient commands:

```bash
# Download models
./demo.sh download

# Start TTS service
./demo.sh service

# Run test client
./demo.sh client

# Test HTTP endpoints
./demo.sh test

# Run full demo
./demo.sh demo

# Show help
./demo.sh help
```

## Using with curl

### Health Check

```bash
curl http://localhost:8080/health
```

### Synthesize Speech

```bash
# Create request JSON
REQUEST=$(cat <<EOF
{
  "topic": "tts.synthesize",
  "data": $(echo '{"text":"Hello, world!","lang":"en"}' | base64 -w0),
  "sender": "curl",
  "reply_to": null
}
EOF
)

# Send request
curl -X POST http://localhost:8080/publish \
  -H "Content-Type: application/json" \
  -d "$REQUEST"
```

## Architecture

```
┌─────────────────┐     HTTP      ┌─────────────────┐
│   HTTP Client   │ ◄──────────► │   TTS Service   │
│  (tts-client)   │               │  (tts-service)  │
└─────────────────┘               └────────┬────────┘
                                           │
                                           ▼
                                  ┌─────────────────┐
                                  │  ONNX Runtime   │
                                  │   (TTS Models)  │
                                  └─────────────────┘
```

## Model Files

The TTS service uses the following models from [Supertone/supertonic](https://huggingface.co/Supertone/supertonic):

| File | Description |
|------|-------------|
| `tokenizer_config.json` | Tokenizer configuration |
| `config.json` | Model configuration |
| `tokenizer.json` | Tokenizer vocabulary |
| `voice_decoder.onnx` | Voice decoder neural network |
| `latent_denoiser.onnx` | Latent denoising network |
| `text_encoder.onnx` | Text encoder neural network |

## License

Apache-2.0

## Related Projects

- [moleculer-rs](https://github.com/primcloud/moleculer-rs) - Progressive microservices framework for Rust
- [Moleculer JS](https://github.com/moleculerjs/moleculer) - Original JavaScript framework
- [Supertone/supertonic](https://huggingface.co/Supertone/supertonic) - Original Supertonic TTS model
