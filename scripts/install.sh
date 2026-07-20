#!/usr/bin/env bash
set -euo pipefail

# IGS MCP Server — Install Script
# Downloads the latest release binary and sets up config.

REPO="ishan-parihar/igs-rust"
INSTALL_DIR="${IGS_INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${IGS_CONFIG_DIR:-$HOME/.config/igs-mcp}"

echo "=== IGS Intelligence Gathering System — Installer ==="
echo ""

# Detect platform (uses Rust target triples to match release artifacts)
ARCH=$(uname -m)
OS=$(uname -s)
case "$OS" in
    Linux)  PLATFORM="x86_64-unknown-linux-musl" ;;
    Darwin) PLATFORM="aarch64-apple-darwin" ;;
    *)      echo "Error: Unsupported OS: $OS"; exit 1 ;;
esac
case "$ARCH" in
    x86_64)
        if [ "$OS" = "Linux" ]; then
            PLATFORM="x86_64-unknown-linux-musl"
        fi
        ;;
    aarch64|arm64)
        if [ "$OS" = "Linux" ]; then
            PLATFORM="aarch64-unknown-linux-musl"
        elif [ "$OS" = "Darwin" ]; then
            PLATFORM="aarch64-apple-darwin"
        fi
        ;;
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

# ── Install Session Hooks (AXI §7) ─────────────────────────────────────────
echo ""
echo "Installing AI agent session hooks..."

# Claude Code session hook
CLAUDE_SETTINGS="$HOME/.claude/settings.json"
if command -v jq &>/dev/null && [ -f "$CLAUDE_SETTINGS" ]; then
    # Check if igs hook already exists
    if jq -e '.hooks.SessionStart[]?.hooks[]?.command == "igs"' "$CLAUDE_SETTINGS" &>/dev/null; then
        echo "  ✓ Claude Code session hook already installed"
    else
        # Backup and add hook
        cp "$CLAUDE_SETTINGS" "${CLAUDE_SETTINGS}.bak.$(date +%s)"
        jq '.hooks.SessionStart += [{"matcher":"","hooks":[{"type":"command","command":"igs"}]}]' \
            "$CLAUDE_SETTINGS" > "${CLAUDE_SETTINGS}.tmp" && mv "${CLAUDE_SETTINGS}.tmp" "$CLAUDE_SETTINGS"
        echo "  ✓ Claude Code session hook installed to $CLAUDE_SETTINGS"
    fi
else
    echo "  → Claude Code: Add this to ~/.claude/settings.json:"
    echo '    {"hooks":{"SessionStart":[{"matcher":"","hooks":[{"type":"command","command":"igs"}]}]}}'
fi

# Codex session hook
CODEX_DIR="$HOME/.codex"
if [ -d "$CODEX_DIR" ]; then
    CODEX_HOOKS="$CODEX_DIR/hooks.json"
    if [ -f "$CODEX_HOOKS" ] && jq -e '.SessionStart == "igs"' "$CODEX_HOOKS" &>/dev/null; then
        echo "  ✓ Codex session hook already installed"
    else
        # Add/update hook
        if [ -f "$CODEX_HOOKS" ]; then
            cp "$CODEX_HOOKS" "${CODEX_HOOKS}.bak.$(date +%s)"
            jq '.SessionStart = "igs"' "$CODEX_HOOKS" > "${CODEX_HOOKS}.tmp" && mv "${CODEX_HOOKS}.tmp" "$CODEX_HOOKS"
        else
            echo '{"SessionStart":"igs"}' > "$CODEX_HOOKS"
        fi
        echo "  ✓ Codex session hook installed to $CODEX_HOOKS"
        # Ensure hooks feature is enabled
        CODEX_CONFIG="$CODEX_DIR/config.toml"
        if [ -f "$CODEX_CONFIG" ] && ! grep -q 'hooks = true' "$CODEX_CONFIG"; then
            echo -e '\n[features]\nhooks = true' >> "$CODEX_CONFIG"
            echo "  ✓ Enabled hooks in $CODEX_CONFIG"
        fi
    fi
else
    echo "  → Codex: Create ~/.codex/hooks.json with {"SessionStart":"igs"}"
fi

# OpenCode session hook
OPENCODE_DIR="$HOME/.config/opencode/plugins"
if [ -d "$HOME/.config/opencode" ]; then
    mkdir -p "$OPENCODE_DIR"
    if [ -f "$OPENCODE_DIR/igs.ts" ]; then
        echo "  ✓ OpenCode session hook already installed"
    else
        cat > "$OPENCODE_DIR/igs.ts" << 'OPENCODE_PLUGIN'
export default {
  name: "igs",
  onSessionStart: async () => {
    const { execSync } = require("child_process");
    return execSync("igs").toString();
  },
};
OPENCODE_PLUGIN
        echo "  ✓ OpenCode session hook installed to $OPENCODE_DIR/igs.ts"
    fi
else
    echo "  → OpenCode: Create ~/.config/opencode/plugins/igs.ts (see README)"
fi

# Install SKILL.md for agents that don't support hooks
SKILL_DIR="$HOME/.agents/skills/igs"
mkdir -p "$SKILL_DIR"
if [ ! -f "$SKILL_DIR/SKILL.md" ]; then
    if command -v curl &>/dev/null; then
        curl -fsSL "https://raw.githubusercontent.com/ishan-parihar/igs-rust/master/SKILL.md" \
            -o "$SKILL_DIR/SKILL.md" 2>/dev/null && \
            echo "  ✓ Skill installed to $SKILL_DIR/SKILL.md" || \
            echo "  → Skill download failed (non-critical)"
    fi
else
    echo "  ✓ Skill already installed at $SKILL_DIR/SKILL.md"
fi
