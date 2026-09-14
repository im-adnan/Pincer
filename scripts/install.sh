#!/usr/bin/env bash

set -e

# Pincer Engine Installation Script
# This script downloads the latest macOS ARM64 release and installs it.

REPO="grabbit/pincer-engine"
BIN_NAME="pincer"

echo "Installing Pincer Engine..."

# Detect OS
OS="$(uname -s)"
if [ "$OS" != "Darwin" ]; then
    echo "Error: This install script currently only supports macOS (Darwin)."
    echo "Detected: $OS"
    exit 1
fi

# Detect Architecture
ARCH="$(uname -m)"
if [ "$ARCH" != "arm64" ]; then
    echo "Warning: You are running on $ARCH."
    echo "This installation will download the ARM64 binary. If you are on an Intel Mac,"
    echo "it will run via Rosetta 2."
fi

# Fetch the latest release data from GitHub API
echo "Fetching latest release information..."
LATEST_RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
DOWNLOAD_URL=$(curl -s $LATEST_RELEASE_URL | grep "browser_download_url.*pincer-macos.zip" | cut -d : -f 2,3 | tr -d \" | tr -d " ")

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find the download URL for the latest macOS release."
    exit 1
fi

# Download the zip file
TEMP_DIR=$(mktemp -d)
ZIP_FILE="$TEMP_DIR/pincer.zip"

echo "Downloading from $DOWNLOAD_URL..."
curl -sL "$DOWNLOAD_URL" -o "$ZIP_FILE"

# Extract
echo "Extracting..."
unzip -q "$ZIP_FILE" -d "$TEMP_DIR"

# Install
INSTALL_DIR="/usr/local/bin"

# Check if we can write to the install dir, otherwise fallback to ~/.local/bin
if [ ! -w "$INSTALL_DIR" ]; then
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
    echo "Warning: /usr/local/bin is not writable. Installing to $INSTALL_DIR instead."
    echo "Make sure $INSTALL_DIR is in your PATH."
fi

mv "$TEMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

# Cleanup
rm -rf "$TEMP_DIR"

echo "✅ Successfully installed $BIN_NAME to $INSTALL_DIR!"
echo "Run 'pincer --help' to get started."
