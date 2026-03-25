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

# 3. Build the UI (Swift app)
if command -v swift >/dev/null 2>&1; then
    echo "🖥️  Building xclaude UI..."
    (cd xclaude-app && swift build)
    # Create a minimal .app bundle so macOS launches it as a GUI app
    UI_BIN="xclaude-app/.build/arm64-apple-macosx/debug/XClaudeApp"
    if [ -f "$UI_BIN" ]; then
        APP_DIR="$HOME/.local/bin/XClaudeApp.app/Contents/MacOS"
        mkdir -p "$APP_DIR"
        cp "$UI_BIN" "$APP_DIR/XClaudeApp"
        cat > "$HOME/.local/bin/XClaudeApp.app/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>XClaudeApp</string>
    <key>CFBundleIdentifier</key>
    <string>com.xclaude.app</string>
    <key>CFBundleName</key>
    <string>XClaude</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <false/>
</dict>
</plist>
PLIST
    fi
else
    echo "⚠️  Swift not found — skipping UI build. Install Xcode Command Line Tools to enable 'xclaude ui'."
fi

# 4. Create install directories and copy the binary
echo "📦 Installing xclaude locally to ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/xclaude ~/.local/bin/xclaude


# 6. Add to PATH automatically
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

# 7. Provide final instructions
echo ""
echo "✅ Success! xclaude has been perfectly installed."
echo ""
echo "⚠️  Please restart your terminal or run 'source ~/.zshrc' (or ~/.bashrc) to apply PATH changes."
echo "You can now use 'claude' or 'xclaude' normally, and all hooks will stream to your RPC endpoints!"
