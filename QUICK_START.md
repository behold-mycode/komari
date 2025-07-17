# Komari MapleStory Bot - Quick Start Guide

## 🚀 Easy Ways to Run Komari

### Option 1: Terminal Script (Recommended)
```bash
cd /Users/me/Documents/Projects/komari/komari_fork
./run_komari.sh
```

### Option 2: Command Alias
First time setup:
```bash
./setup_alias.sh
source ~/.zshrc
```

Then just type:
```bash
komari
```

### Option 3: Desktop App
Create a desktop app icon:
```bash
./create_app.sh
```
Then double-click the Komari app on your Desktop.

### Option 4: Direct Command
```bash
cd /Users/me/Documents/Projects/komari/komari_fork
cargo run --release --bin ui
```

## 🎮 Usage

### Important Settings
- **Arduino Device**: `/dev/cu.usbmodemHIDFG1`
- **Screen Capture**: External monitor at (1770, 270)
- **Game Resolution**: 1366x768 recommended

### Hotkeys
- **J** - Mark platform start position
- **K** - Mark platform end position  
- **L** - Add platform to path
- **Comma (,)** - Additional hotkey

### First Time Setup
1. Grant accessibility permissions when prompted
2. Connect Arduino device (if using hardware input)
3. Configure MapleStory to run at 1366x768
4. Position game on external monitor

## 🔧 Troubleshooting

### If UI appears unformatted:
```bash
cd ui && npm install
cd .. && cargo clean && cargo build --release
```

### If keyboard capture fails:
1. System Preferences → Security & Privacy → Privacy → Accessibility
2. Add Terminal (or Komari app) and enable permissions
3. Restart the application

### If Arduino not detected:
1. Check device is connected to `/dev/cu.usbmodemHIDFG1`
2. Verify Arduino sketch is uploaded
3. Try unplugging and reconnecting

## 📝 Notes
- Always run in release mode for best performance
- The bot will create a `local.db` file for settings
- Logs are saved to `log.txt` in the same directory