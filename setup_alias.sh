#!/bin/bash
# Setup command-line alias for Komari

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
SHELL_CONFIG=""

# Detect shell configuration file
if [ -f "$HOME/.zshrc" ]; then
    SHELL_CONFIG="$HOME/.zshrc"
elif [ -f "$HOME/.bash_profile" ]; then
    SHELL_CONFIG="$HOME/.bash_profile"
elif [ -f "$HOME/.bashrc" ]; then
    SHELL_CONFIG="$HOME/.bashrc"
else
    echo "Creating .zshrc file..."
    touch "$HOME/.zshrc"
    SHELL_CONFIG="$HOME/.zshrc"
fi

# Check if alias already exists
if grep -q "alias komari=" "$SHELL_CONFIG" 2>/dev/null; then
    echo "❌ Komari alias already exists in $SHELL_CONFIG"
    echo "To update it, please remove the existing alias first."
else
    # Add alias to shell config
    echo "" >> "$SHELL_CONFIG"
    echo "# Komari MapleStory Bot alias" >> "$SHELL_CONFIG"
    echo "alias komari='cd $SCRIPT_DIR && ./run_komari.sh'" >> "$SHELL_CONFIG"
    
    echo "✅ Alias added to $SHELL_CONFIG"
    echo ""
    echo "To use the alias, either:"
    echo "1. Open a new terminal window, or"
    echo "2. Run: source $SHELL_CONFIG"
    echo ""
    echo "Then you can simply type 'komari' to launch the bot!"
fi