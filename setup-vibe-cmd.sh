mkdir -p ~/.local/bin
cat > ~/.local/bin/vibe <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

REPO="${VIBE_KANBAN_REPO:-$HOME/dev/vibe-kanban}"
cd "$REPO"

exec cargo run -p tui --bin vibe-kanban-tui -- "$@"
EOF
chmod +x ~/.local/bin/vibe

# Ensure it's on PATH (add to your shell rc if needed)
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$HOME/.local/bin"; then
  echo 'Add this to your shell rc: export PATH="$HOME/.local/bin:$PATH"'
fi
