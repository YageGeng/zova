#!/bin/bash
# Deploy script for Zova PDF Layout Analyzer

set -e

echo "🚀 Building Zova PDF Layout Analyzer..."

# Build WASM module
echo "📦 Building WASM module..."
cd crates/wasm
cargo build --target wasm32-unknown-unknown --release
wasm-pack build --target web --out-dir ../../web/pkg
cd ../..

# Check if build succeeded
if [ ! -f "web/pkg/zova_wasm_bg.wasm" ]; then
    echo "❌ WASM build failed!"
    exit 1
fi

echo "✅ WASM build successful!"

# Start server
echo "🌐 Starting web server on http://localhost:8080"
cd web
python3 -m http.server 8080