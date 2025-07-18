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

# Arduino server variables
ARDUINO_SERVER_PID_FILE="$SCRIPT_DIR/.arduino_server.pid"
ARDUINO_SERVER_PORT=5001
ARDUINO_SCRIPT="$SCRIPT_DIR/examples/python/arduino_example.py"

# Function to check if Arduino server is running
check_arduino_server() {
    if [ -f "$ARDUINO_SERVER_PID_FILE" ]; then
        local pid=$(cat "$ARDUINO_SERVER_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            return 0  # Server is running
        else
            rm -f "$ARDUINO_SERVER_PID_FILE"  # Clean up stale PID file
            return 1  # Server is not running
        fi
    fi
    return 1  # PID file doesn't exist
}

# Function to stop Arduino server
stop_arduino_server() {
    if [ -f "$ARDUINO_SERVER_PID_FILE" ]; then
        local pid=$(cat "$ARDUINO_SERVER_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${YELLOW}🔌 Stopping Arduino server (PID: $pid)...${NC}"
            kill "$pid"
            sleep 2
            
            # Force kill if still running
            if ps -p "$pid" > /dev/null 2>&1; then
                echo -e "${RED}Force killing Arduino server...${NC}"
                kill -9 "$pid"
            fi
        fi
        rm -f "$ARDUINO_SERVER_PID_FILE"
    fi
}

# Function to start Arduino server
start_arduino_server() {
    if check_arduino_server; then
        echo -e "${YELLOW}⚠️  Arduino server is already running${NC}"
        return 0
    fi
    
    # Check if port is in use by another process
    if lsof -Pi :$ARDUINO_SERVER_PORT -sTCP:LISTEN -t >/dev/null; then
        echo -e "${RED}❌ Port $ARDUINO_SERVER_PORT is already in use by another process!${NC}"
        echo "Please stop any existing Arduino servers before running Komari."
        return 1
    fi
    
    if [ ! -f "$ARDUINO_SCRIPT" ]; then
        echo -e "${RED}❌ Arduino script not found: $ARDUINO_SCRIPT${NC}"
        return 1
    fi
    
    echo -e "${YELLOW}🔌 Starting Arduino gRPC server...${NC}"
    python3 "$ARDUINO_SCRIPT" > /dev/null 2>&1 &
    local pid=$!
    echo "$pid" > "$ARDUINO_SERVER_PID_FILE"
    
    # Wait a moment for server to start
    sleep 3
    
    # Verify server started successfully
    if ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Arduino server started successfully (PID: $pid)${NC}"
        return 0
    else
        echo -e "${RED}❌ Failed to start Arduino server${NC}"
        rm -f "$ARDUINO_SERVER_PID_FILE"
        return 1
    fi
}

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}🧹 Cleaning up...${NC}"
    stop_arduino_server
    exit 0
}

# Set up signal handlers for cleanup
trap cleanup SIGINT SIGTERM EXIT

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
    
    # Check if Python3 is available for Arduino server
    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}❌ Error: Python3 is not installed!${NC}"
        echo "Python3 is required for the Arduino gRPC server."
        exit 1
    fi
    
    # Start Arduino server
    if ! start_arduino_server; then
        echo -e "${RED}❌ Failed to start Arduino server. Exiting.${NC}"
        exit 1
    fi
    
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