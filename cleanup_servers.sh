#!/bin/bash
# Cleanup script for Komari servers

echo "🧹 Cleaning up any running Komari servers..."

# Kill any processes using port 5001 (Arduino server)
if lsof -Pi :5001 -sTCP:LISTEN -t >/dev/null; then
    echo "🔌 Stopping processes on port 5001..."
    lsof -Pi :5001 -sTCP:LISTEN -t | xargs kill
    sleep 2
    
    # Force kill if still running
    if lsof -Pi :5001 -sTCP:LISTEN -t >/dev/null; then
        echo "🔥 Force killing processes on port 5001..."
        lsof -Pi :5001 -sTCP:LISTEN -t | xargs kill -9
    fi
    echo "✅ Port 5001 cleaned up"
else
    echo "✅ Port 5001 is already free"
fi

# Clean up PID files
if [ -f ".arduino_server.pid" ]; then
    echo "🗑️  Removing stale PID file..."
    rm -f ".arduino_server.pid"
fi

echo "✅ Cleanup complete!"