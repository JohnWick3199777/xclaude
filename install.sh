#!/usr/bin/env bash

set -e

echo "=> Welcome to the xclaude installer!"
echo "--------------------------------------"

# 1. Check if Rust/Cargo is installed
if ! command -v cargo >/dev/null 2>&1; then
    echo "❌ Error: Rust and Cargo are not installed."
    echo "Please install Rust first by running:"
    echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 2. Build the project
echo "🛠️  Building xclaude in release mode..."
cargo build --release

# 3. Create install directories and copy the binary
echo "📦 Installing xclaude locally to ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/xclaude ~/.local/bin/xclaude

# 4. Trigger the built-in installer (creates symlinks)
echo "🔗 Setting up claude wrapper aliases..."
~/.local/bin/xclaude install

# 5. Add to PATH automatically
echo "🔍 Checking PATH configuration..."

if [ -f "$HOME/.zshrc" ]; then
    if ! grep -q "HOME/.local/bin" "$HOME/.zshrc" 2>/dev/null; then
        echo "" >> "$HOME/.zshrc"
        echo "# xclaude PATH" >> "$HOME/.zshrc"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.zshrc"
        echo "   Added ~/.local/bin to ~/.zshrc"
    else
        echo "   ~/.local/bin is already in ~/.zshrc"
    fi
fi

if [ -f "$HOME/.bashrc" ]; then
    if ! grep -q "HOME/.local/bin" "$HOME/.bashrc" 2>/dev/null; then
        echo "" >> "$HOME/.bashrc"
        echo "# xclaude PATH" >> "$HOME/.bashrc"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
        echo "   Added ~/.local/bin to ~/.bashrc"
    else
        echo "   ~/.local/bin is already in ~/.bashrc"
    fi
fi

# 6. Provide final instructions
echo ""
echo "✅ Success! xclaude has been perfectly installed."
echo ""
echo "⚠️  Please restart your terminal or run 'source ~/.zshrc' (or ~/.bashrc) to apply PATH changes."
echo "You can now use 'claude' or 'xclaude' normally, and all hooks will stream to your RPC endpoints!"
