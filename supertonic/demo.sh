#!/bin/bash
# ============================================================================
# Supertonic TTS Demo Script
# ============================================================================
#
# This script demonstrates the TTS microservice with HTTP protocol.
#
# Usage:
#   ./demo.sh [command]
#
# Commands:
#   copy-local  - Copy models from local cache
#   download    - Download ONNX models from Hugging Face
#   service     - Start the TTS service
#   client      - Run the test client
#   demo        - Run a full demo (copy-local + service + client)
#   help        - Show this help message
#
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CACHE_DIR="${SCRIPT_DIR}/cache"
LOCAL_CACHE_BASE="/home/meme/Documentos/NODEAPI/supertonic/.cache/onnx-community/Supertonic-TTS-2-ONNX"
LOCAL_CACHE_ONNX="${LOCAL_CACHE_BASE}/onnx"
ONNX_DIR="${CACHE_DIR}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}"
    echo "========================================"
    echo " $1"
    echo "========================================"
    echo -e "${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

copy_local_models() {
    print_header "Copying Models from Local Cache"
    
    # Check if local cache exists
    if [ ! -d "${LOCAL_CACHE_BASE}" ]; then
        print_error "Local cache not found: ${LOCAL_CACHE_BASE}"
        exit 1
    fi
    
    echo "Local cache base: ${LOCAL_CACHE_BASE}"
    echo "Local cache onnx: ${LOCAL_CACHE_ONNX}"
    echo "Target directory: ${CACHE_DIR}"
    echo ""
    
    # Create cache directory
    mkdir -p "${CACHE_DIR}"
    
    # Copy config files from base directory
    local copied=0
    local missing=0
    
    # Copy config files
    for file in "tokenizer_config.json" "config.json" "tokenizer.json"; do
        src="${LOCAL_CACHE_BASE}/${file}"
        dest="${CACHE_DIR}/${file}"
        
        if [ -f "${src}" ]; then
            cp "${src}" "${dest}"
            local size=$(du -h "${dest}" | cut -f1)
            print_success "Copied: ${file} (${size})"
            copied=$((copied + 1))
        else
            print_warning "Missing: ${file}"
            missing=$((missing + 1))
        fi
    done
    
    # Copy ONNX files (including _data files)
    if [ -d "${LOCAL_CACHE_ONNX}" ]; then
        for file in "${LOCAL_CACHE_ONNX}"/*; do
            if [ -f "${file}" ]; then
                filename=$(basename "${file}")
                dest="${CACHE_DIR}/${filename}"
                cp "${file}" "${dest}"
                local size=$(du -h "${dest}" | cut -f1)
                print_success "Copied: ${filename} (${size})"
                copied=$((copied + 1))
            fi
        done
    else
        print_warning "ONNX directory not found: ${LOCAL_CACHE_ONNX}"
        missing=$((missing + 1))
    fi
    
    echo ""
    print_success "Copied ${copied} files"
    
    if [ ${missing} -gt 0 ]; then
        print_warning "${missing} files missing from local cache"
    fi
    
    echo ""
    echo "Files in cache:"
    ls -lh "${CACHE_DIR}"
}

download_models() {
    print_header "Downloading ONNX Models"
    
    cd "${SCRIPT_DIR}"
    
    # Create cache directory
    mkdir -p "${CACHE_DIR}"
    
    echo "Cache directory: ${CACHE_DIR}"
    echo ""
    
    # Run model downloader
    cargo run --bin model-downloader -- --cache "${CACHE_DIR}"
    
    echo ""
    print_success "Model download complete"
}

start_service() {
    print_header "Starting TTS Service"
    
    cd "${SCRIPT_DIR}"
    
    # Kill any previous instance on port 8080
    fuser -k 8080/tcp 2>/dev/null || true
    sleep 1
    
    # Check if models exist
    if [ ! -f "${ONNX_DIR}/text_encoder.onnx" ]; then
        print_error "Models not found. Run './demo.sh copy-local' or './demo.sh download' first."
        exit 1
    fi
    
    echo "ONNX Directory: ${ONNX_DIR}"
    echo ""
    echo "Starting service on http://0.0.0.0:8080"
    echo "Press Ctrl+C to stop"
    echo ""
    
    cargo run --bin tts-service -- \
        --onnx-dir "${ONNX_DIR}" \
        --voice-style "${ONNX_DIR}/voice_style.json" \
        --address "0.0.0.0:8080" \
        --node-id "tts-demo-node" \
        --output-dir "${SCRIPT_DIR}/cache"
}

run_client() {
    print_header "Running TTS Client"
    
    cd "${SCRIPT_DIR}"
    
    cargo run --bin tts-client
}

test_http() {
    print_header "Testing HTTP Endpoints"
    
    local SERVER_URL="http://localhost:8080"
    
    echo "Testing health endpoint..."
    if curl -s "${SERVER_URL}/health" | jq . 2>/dev/null; then
        print_success "Health check passed"
    else
        print_warning "Health check failed (service may not be running)"
    fi
    
    echo ""
    echo "Testing info endpoint..."
    if curl -s "${SERVER_URL}/info" | jq . 2>/dev/null; then
        print_success "Info check passed"
    else
        print_warning "Info check failed (service may not be running)"
    fi
    
    echo ""
}

full_demo() {
    print_header "Full TTS Demo"
    
    echo "This demo will:"
    echo "  1. Copy models from local cache"
    echo "  2. Start the TTS service"
    echo "  3. Run test client"
    echo ""
    
    # Copy models
    copy_local_models
    
    echo ""
    echo "Starting service in background..."
    
    # Start service in background (output disabled by default)
    cargo run --bin tts-service -- \
        --onnx-dir "${ONNX_DIR}" \
        --voice-style "${ONNX_DIR}/voice_style.json" \
        --address "0.0.0.0:8080" \
        --node-id "tts-demo-node" \
        --output-dir "${SCRIPT_DIR}/cache" &
    
    SERVICE_PID=$!
    
    # Wait for service to start
    echo "Waiting for service to start..."
    sleep 5
    
    # Test endpoints
    test_http
    
    # Run client
    run_client
    
    # Stop service
    echo ""
    echo "Stopping service..."
    kill $SERVICE_PID 2>/dev/null || true
    
    print_success "Demo complete!"
}

show_help() {
    echo "Supertonic TTS Demo Script"
    echo ""
    echo "Usage: ./demo.sh [command]"
    echo ""
    echo "Commands:"
    echo "  copy-local  - Copy models from local cache"
    echo "  download    - Download ONNX models from Hugging Face"
    echo "  service     - Start the TTS service"
    echo "  client      - Run the test client"
    echo "  test        - Test HTTP endpoints"
    echo "  demo        - Run a full demo"
    echo "  help        - Show this help message"
    echo ""
    echo "Examples:"
    echo "  ./demo.sh copy-local  # Copy models from local cache"
    echo "  ./demo.sh download    # Download models from Hugging Face"
    echo "  ./demo.sh service     # Start TTS service"
    echo "  ./demo.sh client      # Test the service"
    echo ""
    echo "Local cache location:"
    echo "  ${LOCAL_CACHE_BASE}"
}

# Main entry point
case "${1:-help}" in
    copy-local)
        copy_local_models
        ;;
    download)
        download_models
        ;;
    service)
        start_service
        ;;
    client)
        run_client
        ;;
    test)
        test_http
        ;;
    demo)
        full_demo
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        print_error "Unknown command: $1"
        show_help
        exit 1
        ;;
esac
