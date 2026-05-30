#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${IGS_INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${IGS_CONFIG_DIR:-$HOME/.config/igs-mcp}"

echo "=== IGS Uninstaller ==="
echo ""

# Remove binary
if [ -f "$INSTALL_DIR/igs" ]; then
    rm "$INSTALL_DIR/igs"
    echo "Removed: $INSTALL_DIR/igs"
fi

# Remove symlink
if [ -L "$INSTALL_DIR/igs-mcp" ]; then
    rm "$INSTALL_DIR/igs-mcp"
    echo "Removed: $INSTALL_DIR/igs-mcp (symlink)"
fi

# Remove Lightpanda binary
if [ -d "$CONFIG_DIR/bin" ]; then
    rm -rf "$CONFIG_DIR/bin"
    echo "Removed: $CONFIG_DIR/bin/ (Lightpanda binary)"
fi

# Ask about config
echo ""
read -p "Remove config directory ($CONFIG_DIR)? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$CONFIG_DIR"
    echo "Removed: $CONFIG_DIR"
else
    echo "Kept: $CONFIG_DIR"
fi

echo ""
echo "=== Uninstall Complete ==="
