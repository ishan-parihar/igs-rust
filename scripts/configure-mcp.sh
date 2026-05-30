#!/usr/bin/env bash
set -euo pipefail

# IGS — Configure MCP server for Claude Desktop / Cursor / OpenClaude

CONFIG_DIR="${IGS_CONFIG_DIR:-$HOME/.config/igs-mcp}"
IGS_BIN=""

echo "=== IGS MCP Configuration Helper ==="
echo ""

# Find the igs binary
if command -v igs &>/dev/null; then
    IGS_BIN=$(command -v igs)
elif [ -f "$HOME/.local/bin/igs" ]; then
    IGS_BIN="$HOME/.local/bin/igs"
elif [ -f "./target/release/igs" ]; then
    IGS_BIN="$(pwd)/target/release/igs"
elif [ -f "./target/debug/igs" ]; then
    IGS_BIN="$(pwd)/target/debug/igs"
else
    echo "Error: Could not find 'igs' binary."
    echo "Run scripts/install.sh first, or build with: cargo build --release"
    exit 1
fi

echo "Found binary: $IGS_BIN"
echo ""

# Generate MCP config
MCP_CONFIG=$(cat <<EOF
{
  "mcpServers": {
    "igs": {
      "command": "$IGS_BIN",
      "args": ["mcp"]
    }
  }
}
EOF
)

echo "MCP configuration (for Claude Desktop, Cursor, etc.):"
echo ""
echo "$MCP_CONFIG"
echo ""

# Detect Claude Desktop config path
CLAUDE_CONFIG=""
case "$(uname -s)" in
    Darwin)
        CLAUDE_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
        ;;
    Linux)
        CLAUDE_CONFIG="$HOME/.config/Claude/claude_desktop_config.json"
        ;;
esac

if [ -n "$CLAUDE_CONFIG" ]; then
    if [ -f "$CLAUDE_CONFIG" ]; then
        echo "Claude Desktop config found at: $CLAUDE_CONFIG"
        read -p "Add IGS to Claude Desktop config? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            # Check if igs is already configured
            if grep -q '"igs"' "$CLAUDE_CONFIG" 2>/dev/null; then
                echo "IGS already configured in Claude Desktop."
            else
                # Merge into existing config
                EXISTING=$(cat "$CLAUDE_CONFIG")
                # Simple merge: add igs to mcpServers
                if echo "$EXISTING" | python3 -c "import sys,json; d=json.load(sys.stdin); d.get('mcpServers',{})" 2>/dev/null; then
                    echo "$EXISTING" | python3 -c "
import sys, json
d = json.load(sys.stdin)
if 'mcpServers' not in d:
    d['mcpServers'] = {}
d['mcpServers']['igs'] = {'command': '$IGS_BIN', 'args': ['mcp']}
json.dump(d, sys.stdout, indent=2)
" > "$CLAUDE_CONFIG"
                    echo "✓ Added IGS to Claude Desktop config"
                else
                    echo "Could not parse existing config. Please add manually."
                fi
            fi
        fi
    else
        echo "Claude Desktop config not found at: $CLAUDE_CONFIG"
        echo "Create it manually with the JSON above."
    fi
fi

echo ""
echo "=== Configuration Complete ==="
echo ""
echo "Next steps:"
echo "  1. Restart Claude Desktop / Cursor"
echo "  2. The IGS tools should appear in your agent's tool list"
echo "  3. Test with: igs status"
