⏺ Komari MapleStory Bot - Session Documentation

Date: July 16-17, 2025Session Duration: Extended debugging sessionStatus: Issues remain
unresolved

🚨 Original Problems Reported

User reported two critical issues with their Komari MapleStory bot fork for macOS:

1. Hotkeys completely non-responsive - J/K/L keys for platform management don't work at
   all
2. Character movement broken - No movement despite Arduino gRPC server running on
   localhost:5001
3. Game detection failure - Coordinates (1770, 270) entered in GUI but no feedback, game
   never detected

User Context

- Playing MapleStory via GeForce NOW (cloud gaming)
- Using coordinate-based screen capture at (1770, 270)
- Arduino server for system-wide USB HID input
- macOS environment
- Multi-monitor setup (coordinates suggest secondary display)

🔍 Investigation Findings

Database Analysis

Found user settings in /local.db:
{
"capture_mode": "BitBltArea",
"capture_x": 1770,
"capture_y": 270,
"input_method": "Rpc",
"input_method_rpc_server_url": "5001",
"platform_start_key": {"key": "J", "enabled": false},
"platform_end_key": {"key": "K", "enabled": false},
"platform_add_key": {"key": "L", "enabled": false}
}

Critical Discovery: All hotkeys are disabled in database (enabled: false)

Threading Architecture Mapped

UI Thread (main)
├── Backend Thread (spawned in context.rs:172)
│   ├── Update loop (30 FPS)
│   ├── KeyReceiver polling
│   └── RPC communication
└── CGEventTap Thread (spawned in macos/mod.rs:34)
├── CGEventTap creation
├── Event callback processing
└── Core Foundation run loop

RPC System Status

- ✅ URL formatting working: "5001" → "http://localhost:5001"
- ✅ Arduino server connecting successfully
- ✅ End-to-end gRPC communication verified

Screen Capture Issues

- Coordinate validation too strict for multi-monitor
- BitBltArea handle selection disabled in request_handler.rs:404-407
- No user feedback when coordinates fail validation
- Display detection hardcoded to primary display (index 0)

🛠 Attempted Fixes

1. RPC URL Formatting ✅ WORKING

File: backend/src/rpc.rs
- Added format_rpc_url() function
- Converts "5001" to "http://localhost:5001" automatically
- Result: RPC connection successful

2. Database Location Unification ✅ WORKING

File: backend/src/database.rs
- Fixed inconsistent database paths between debug/release
- Unified to project root location
- Result: Settings persist correctly

3. CGEventTap Threading Fix ❌ FAILED

File: platforms/src/macos/keys.rs
- Attempted "safer" Core Foundation event loop with timeouts
- Added panic handling and mutex protection
- Result: Crash still occurs (mutex lock failed: Invalid argument)

4. Multi-Monitor Coordinate Support ⚠️ PARTIAL

Files: platforms/src/macos/handle.rs, backend/src/bridge.rs
- Added display detection for coordinates (1770, 270)
- Enhanced coordinate validation logic
- Result: Not fully tested due to other blocking issues

💔 What Didn't Work & Why

Hotkeys Still Broken

Root Cause: Database has "enabled": false for all hotkeys
- Even with CGEventTap working, hotkeys won't trigger
- UI shows hotkey configuration but doesn't reflect enabled state
- Fix Required: Enable hotkeys in database + UI indication

Movement Still Broken

Root Cause: Arduino input method not being used
- RPC connection works but movement commands not being sent
- Backend may not be properly routing movement to Arduino
- Investigation Required: Trace movement command flow

Game Detection Still Broken

Root Cause: Coordinate validation and multi-monitor issues
- Coordinates (1770, 270) likely valid for secondary display
- System validates against primary display only
- No user feedback about validation status
- Fix Required: Proper multi-monitor bounds checking

📊 Current System State

What's Working ✅

- Application builds and starts
- RPC connection to Arduino server
- Database persistence
- Basic UI functionality
- Settings can be entered (though no feedback)

What's Broken ❌

- Hotkeys completely non-responsive
- Character movement commands
- Game detection/screen capture
- No user feedback for coordinate validation
- Crash on application exit

Log Evidence

[2025-07-17T03:52:44.868813000Z INFO backend::rpc] Attempting to connect to RPC server:
http://localhost:5001 (formatted from: 5001)
[2025-07-17T03:52:44.870152000Z INFO backend::rpc] Successfully connected to RPC server:
http://localhost:5001

🔑 Key Insights for Next Session

Priority Issues (in order)

1. Enable hotkeys in database - Simple database update needed
2. Fix coordinate validation - Implement proper multi-monitor support
3. Debug movement command flow - Trace why Arduino RPC isn't receiving movement
4. Add user feedback - Show coordinate validation and hotkey status in UI

Technical Debt

- Threading architecture needs simplification
- Too many async/sync boundaries in key pipeline
- Error handling and user feedback severely lacking
- Multi-monitor support incomplete

Files That Need Attention

- backend/src/database.rs - Enable hotkeys
- backend/src/request_handler.rs - Fix BitBltArea handle selection
- platforms/src/macos/screenshot.rs - Multi-monitor coordinate validation
- ui/src/settings.rs - Add coordinate validation feedback

Working Arduino Test Setup

Arduino gRPC server can be started with:
cd examples/python && python3 arduino_example.py

End-to-end testing verified with:
cd examples/python && python3 test_rpc_client.py

📋 Recommendations for Next Session

1. Start with simple wins - Enable hotkeys in database first
2. Test incrementally - Fix one issue, test, then move to next
3. Focus on user feedback - Add validation status to UI
4. Avoid architectural changes - Fix immediate issues before refactoring
5. Use working Arduino setup - Leverage existing test infrastructure

Success Criteria: User can press J/K/L hotkeys, see coordinate validation feedback, and
observe character movement in game.
