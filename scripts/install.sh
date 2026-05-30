#!/usr/bin/env bash
set -euo pipefail

# IGS MCP Server — Install Script
# Downloads the latest release binary and sets up config.

REPO="ishan-parihar/igs-rust-mcp"
INSTALL_DIR="${IGS_INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${IGS_CONFIG_DIR:-$HOME/.config/igs-mcp}"

echo "=== IGS Intelligence Gathering System — Installer ==="
echo ""

# Detect platform
ARCH=$(uname -m)
OS=$(uname -s)
case "$OS" in
    Linux)  PLATFORM="x86_64-linux-musl" ;;
    Darwin) PLATFORM="x86_64-macos" ;;
    *)      echo "Error: Unsupported OS: $OS"; exit 1 ;;
esac
case "$ARCH" in
    x86_64)  ;;
    aarch64|arm64) PLATFORM="aarch64-${PLATFORM#*-}" ;;
    *)       echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
esac

echo "Platform: $PLATFORM"
echo "Install dir: $INSTALL_DIR"
echo "Config dir: $CONFIG_DIR"
echo ""

# Get latest release
echo "Fetching latest release..."
LATEST=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST" ]; then
    echo "Error: Could not fetch latest release"
    exit 1
fi
echo "Latest version: $LATEST"
echo ""

# Download
TARBALL="igs-${LATEST#v}-${PLATFORM}.tar.gz"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/${LATEST}/${TARBALL}"
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo "Downloading $DOWNLOAD_URL ..."
curl -L -o "$TMPDIR/$TARBALL" "$DOWNLOAD_URL"
echo "Downloaded $(du -h "$TMPDIR/$TARBALL" | cut -f1)"
echo ""

# Extract
echo "Extracting..."
tar -xzf "$TMPDIR/$TARBALL" -C "$TMPDIR"

# Install binary
mkdir -p "$INSTALL_DIR"
cp "$TMPDIR/igs" "$INSTALL_DIR/igs"
chmod +x "$INSTALL_DIR/igs"

# Create backward-compatible symlink
ln -sf igs "$INSTALL_DIR/igs-mcp"

echo "Installed: $INSTALL_DIR/igs"
echo "Symlink:   $INSTALL_DIR/igs-mcp -> igs"
echo ""

# Bootstrap config
if [ ! -d "$CONFIG_DIR" ]; then
    echo "Creating config directory: $CONFIG_DIR"
    mkdir -p "$CONFIG_DIR"
    # Config files will be auto-bootstrapped on first run
    echo "Config will be created on first run."
else
    echo "Config directory exists: $CONFIG_DIR"
fi
echo ""

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo "⚠  $INSTALL_DIR is not in your PATH."
    echo "   Add this to your shell profile:"
    echo "   export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
fi

# Verify
echo "Verifying..."
if "$INSTALL_DIR/igs" --version >/dev/null 2>&1; then
    VERSION=$("$INSTALL_DIR/igs" --version 2>/dev/null)
    echo "✓ $VERSION"
else
    echo "✗ Binary verification failed"
    exit 1
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Quick start:"
echo "  igs status                          # Show system status"
echo "  igs mcp                             # Start MCP server (for AI agents)"
echo "  igs news fetch --pools GLOBAL_TECH_CYBER --limit 10"
echo "  igs --help                          # Show all commands"
echo ""
echo "MCP config (Claude Desktop / Cursor):"
echo '  {'
echo '    "mcpServers": {'
echo '      "igs": {'
echo "        \"command\": \"$INSTALL_DIR/igs\","
echo '        "args": ["mcp"]'
echo '      }'
echo '    }'
echo '  }'
