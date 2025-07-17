#!/bin/bash
# Create macOS Application Bundle for Komari

APP_NAME="Komari"
APP_DIR="$HOME/Desktop/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "Creating macOS app bundle for Komari..."

# Create directory structure
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

# Create the executable script
cat > "$MACOS_DIR/$APP_NAME" << 'EOF'
#!/bin/bash
# Get the directory where this app is located
APP_DIR="$( cd "$( dirname "$0" )" && cd ../.. && pwd )"
KOMARI_DIR="KOMARI_PATH_PLACEHOLDER"

# Open Terminal and run Komari
osascript <<END
tell application "Terminal"
    activate
    do script "cd '$KOMARI_DIR' && ./run_komari.sh"
end tell
END
EOF

# Replace placeholder with actual path
sed -i '' "s|KOMARI_PATH_PLACEHOLDER|$SCRIPT_DIR|g" "$MACOS_DIR/$APP_NAME"

# Make it executable
chmod +x "$MACOS_DIR/$APP_NAME"

# Create Info.plist
cat > "$CONTENTS_DIR/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.komari.maplestory</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>Komari MapleStory Bot</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.games</string>
</dict>
</plist>
EOF

# Create a simple icon (you can replace this with a proper icon later)
cat > "$RESOURCES_DIR/AppIcon.icns" << EOF
(Binary icon data would go here - using placeholder for now)
EOF

echo "✅ macOS app created at: $APP_DIR"
echo ""
echo "You can now:"
echo "1. Double-click the Komari app on your Desktop to launch"
echo "2. Drag it to your Applications folder if desired"
echo "3. Add it to your Dock for quick access"