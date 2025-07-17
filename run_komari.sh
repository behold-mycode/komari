#!/bin/bash
# Komari MapleStory Bot Launcher

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo -e "${GREEN}🎮 Starting Komari MapleStory Bot...${NC}"
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Error: Rust/Cargo is not installed!${NC}"
    echo "Please install Rust from: https://rustup.rs/"
    exit 1
fi

# Check if node is installed (for CSS compilation)
if ! command -v node &> /dev/null; then
    echo -e "${YELLOW}⚠️  Warning: Node.js is not installed!${NC}"
    echo "CSS may not compile properly without Node.js"
    echo ""
fi

# Check if npm dependencies are installed
if [ ! -d "ui/node_modules" ]; then
    echo -e "${YELLOW}📦 Installing Node.js dependencies...${NC}"
    cd ui && npm install && cd ..
    echo ""
fi

# Build and run in release mode for best performance
echo -e "${YELLOW}🔨 Building application (this may take a moment)...${NC}"
cargo build --release --bin ui

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✅ Build successful!${NC}"
    echo ""
    echo -e "${YELLOW}📋 Important Notes:${NC}"
    echo "  • Grant accessibility permissions if prompted"
    echo "  • Arduino device: /dev/cu.usbmodemHIDFG1"
    echo "  • Screen capture: (1770, 270)"
    echo "  • Hotkeys: J (start), K (end), L (add platform)"
    echo ""
    echo -e "${GREEN}🚀 Launching Komari...${NC}"
    echo "----------------------------------------"
    
    # Run the application
    cargo run --release --bin ui
else
    echo -e "${RED}❌ Build failed! Please check the error messages above.${NC}"
    exit 1
fi