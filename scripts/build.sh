#!/bin/bash
# Build and release Wolfy locally
# Usage: ./scripts/build.sh [--release]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET="x86_64-pc-windows-gnu"
DIST_DIR="$PROJECT_DIR/dist"

cd "$PROJECT_DIR"

# Default to release build
BUILD_TYPE="release"
CARGO_FLAGS="--release"

if [[ "$1" == "--debug" ]]; then
    BUILD_TYPE="debug"
    CARGO_FLAGS=""
fi

echo "=== Building Wolfy ($BUILD_TYPE) ==="
cargo build $CARGO_FLAGS --target "$TARGET"

echo "=== Copying to dist/ ==="
mkdir -p "$DIST_DIR"

if [[ "$BUILD_TYPE" == "release" ]]; then
    cp "target/$TARGET/release/wolfy.exe" "$DIST_DIR/"
else
    cp "target/$TARGET/debug/wolfy.exe" "$DIST_DIR/"
fi

# Copy config files (always overwrite to deploy updates)
for f in default.rasi launcher.rasi theme_picker.rasi wallpaper_picker.rasi tasks.toml; do
    if [[ -f "$PROJECT_DIR/$f" ]]; then
        cp "$PROJECT_DIR/$f" "$DIST_DIR/"
    fi
done

# Copy startup script
cp "$SCRIPT_DIR/install-startup.ps1" "$DIST_DIR/"

echo "=== Done ==="
ls -lh "$DIST_DIR/wolfy.exe"
echo ""
echo "Build complete: dist/wolfy.exe"
