# Komari macOS Migration Analysis Journal

## Overview
Systematic analysis of the Windows-to-macOS port migration, following CLAUDE.md methodology.

## Key Findings

### 1. Root Cause of WebView Crash IDENTIFIED

**Original (komari_fork) - Working Windows Code:**
```rust
// ui/src/inputs/keys.rs:94-106 (SIMPLE & SYNCHRONOUS)
onkeydown: move |e: Event<KeyboardData>| async move {
    e.prevent_default();
    if let Some(key) = map_key(e.key()) {
        if let Some(input) = input_element().as_ref() {
            let _ = input.set_focus(false).await;  // ONE async operation
        }
        has_error.set(false);     // Simple signal updates
        on_active(false);
        on_value(Some(key));
    }
},
```

**Modified (komari-master) - BROKEN with Complex Async:**
```rust
// ui/src/inputs/keys.rs:78-100 (COMPLEX ASYNC RESOURCE PATTERN)
let mut key_processor = use_resource(move || async move {
    if let Some(key) = pending_key() {
        pending_key.set(None);
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;  // UNNECESSARY DELAY
        on_value(Some(key));
        on_active(false);
        key_processing.set(false);
    }
});

use_effect(move || {  // ADDITIONAL COMPLEXITY
    if pending_key().is_some() && !key_processing() {
        key_processing.set(true);
        key_processor.restart();  // RESOURCE RESTART = RACE CONDITION
    }
});
```

### 2. Critical Analysis

**The Problem:** The "fix" actually CREATED the race condition by:
1. Adding complex async resource patterns
2. Introducing multiple signal states (key_processing, pending_key)
3. Using resource.restart() which creates WebView threading violations
4. Adding unnecessary tokio::time::sleep delays

**What Actually Happened:**
- Original Windows code: Simple async event → one WebView operation → signal updates
- Modified macOS code: Complex async chains → resource restarts → WebView race conditions

### 3. macOS Platform Support Added

**New Dependencies in platforms/Cargo.toml:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
screenshots = "0.8"
core-graphics = "0.23"
core-foundation = "0.9"
```

**New Platform Module Structure:**
- `platforms/src/lib.rs` - Added `#[cfg(target_os = "macos")] pub mod macos;`
- `platforms/src/macos/` - Complete macOS implementation with:
  - `bitblt.rs` - macOS screen capture
  - `error.rs` - macOS-specific error handling
  - `handle.rs` - macOS window/process handling
  - `keys.rs` - macOS keyboard input system
  - `screenshot.rs` - macOS screenshot functionality
  - `mod.rs` - Platform initialization and event loop

### 4. macOS Platform Implementation Quality

**Good Additions:**
- Proper conditional compilation for macOS
- Native macOS APIs (Core Graphics, Core Foundation)
- Screenshot functionality implemented
- Platform-specific error handling

**Concerns:**
- No Apple onnxruntime binary (`onnxruntime-osx-arm64.tgz` present but not extracted)
- Missing macOS-specific resources/binaries setup

### 5. Migration Complexity Assessment

**Simple Migration Required:**
1. **Copy working original keyboard code** from komari_fork to komari-master
2. **Remove complex async resource patterns** entirely  
3. **Keep macOS platform additions** (they're actually good)
4. **Setup macOS onnxruntime** properly

**Not Required:**
- Complex async "WebView-safe" patterns (they cause the problem)
- Resource restart mechanisms
- Defensive error boundaries with panic::catch_unwind
- Multiple processing state signals

## Migration Strategy

### Phase 1: Clean Slate Reset
1. Copy `ui/src/inputs/keys.rs` from komari_fork (original working version)
2. Keep all macOS platform code additions from komari-master
3. Ensure macOS dependencies remain in Cargo.toml

### Phase 2: macOS Resource Setup  
1. Extract/setup onnxruntime-osx-arm64.tgz properly
2. Verify platform initialization works

### Phase 3: Testing
1. Manual keyboard input testing on macOS
2. Verify no WebView crashes
3. Confirm macOS-specific functionality works

## Critical Mistakes to Avoid
1. **DO NOT** add complex async resource patterns
2. **DO NOT** use resource.restart() in keyboard handlers  
3. **DO NOT** add unnecessary tokio::time::sleep delays
4. **DO NOT** introduce additional signal state management
5. **KEEP IT SIMPLE** - the original Windows code pattern works

## CRITICAL ARDUINO RPC FINDINGS

### 1. Proto Files - IDENTICAL ✅
Both komari_fork and komari-master have identical `input.proto` files. This is good - the RPC interface definition is correct.

### 2. Arduino Python Script - MAJOR DIFFERENCES FOUND ❌

**Original (komari_fork) Issues:**
- Hard-coded Windows COM port: `serial.Serial("COM6")`
- Timer-based key management with `self.timers_map` - overly complex
- No macOS port detection
- No error handling for serial connection

**Modified (komari-master) - Better but Incomplete:**
- ✅ Added macOS Arduino port detection with HID preference
- ✅ Added fallback to test mode if no Arduino found  
- ✅ Better error handling
- ✅ Removed complex timer management - uses simple sleep
- ⚠️  Still missing some integration pieces

**Key Improvements in Master:**
```python
# Smart port detection for macOS
hid_ports = glob.glob('/dev/tty.usbmodem*HID*')  # Matches your /dev/cu.usbmodemHIDFG1
other_ports = glob.glob('/dev/tty.usbmodem*') + glob.glob('/dev/tty.usbserial*')

# Test mode fallback
if all_ports:
    arduino_port = all_ports[0]
    serial_conn = serial.Serial(arduino_port)
else:
    print("No Arduino found - running in test mode")
    serial_conn = None
```

**CRITICAL ISSUE IDENTIFIED:** The modified script looks for `/dev/tty.usbmodem*` but your Arduino is at `/dev/cu.usbmodemHIDFG1`. The script should also check `/dev/cu.*` patterns.

### 3. Backend Bridge Implementation - SIGNIFICANT DIFFERENCES ⚠️

**Original (komari_fork):**
```rust
use platforms::windows::{
    self, BitBltCapture, Frame, Handle, KeyInputKind, KeyKind, Keys, WgcCapture, WindowBoxCapture,
};
```

**Modified (komari-master) - Has macOS Support:**
```rust
#[cfg(windows)]
use platforms::windows::{
    self, BitBltCapture, Frame, Handle, KeyInputKind, KeyKind, Keys, WgcCapture, WindowBoxCapture,
};

#[cfg(target_os = "macos")]
use platforms::macos::{
    self, BitBltCapture, Frame, Handle, KeyKind, KeyInputKind, Keys, screenshot::ScreenshotCapture,
};
```

**✅ GOOD:** The Rust backend properly supports conditional compilation for macOS.

### 4. Settings/Configuration - IDENTICAL ✅
Both versions have:
- RPC server URL setting
- Platform hotkeys (Add platform, Mark platform start/end)
- Same UI structure for configuring Arduino/RPC connection

### 5. Test Infrastructure - MISSING FROM FORK ❌
komari-master has `/examples/python/test_rpc_client.py` which provides:
- End-to-end RPC testing
- Verification of all 76 key mappings
- Connection validation
- Comprehensive test suite

**This was likely created to debug the exact issues you encountered!**

## CRITICAL MIGRATION FINDINGS

### Issues with Current komari-master Arduino Implementation:

1. **Arduino Port Detection Bug:**
   - Script looks for `/dev/tty.usbmodem*HID*`
   - Your Arduino is at `/dev/cu.usbmodemHIDFG1`
   - Missing `/dev/cu.*` pattern matching

2. **Missing Integration:**
   - Python RPC server exists but needs proper startup integration
   - Backend supports RPC but connection might not be established properly

3. **WebView Crash Issue:**
   - Unrelated to Arduino - caused by complex async keyboard handling in UI
   - Should be fixed by copying original simple code from fork

### What Actually Works in Master:
- ✅ macOS platform support in Rust backend
- ✅ Improved Arduino Python script structure  
- ✅ Better error handling and test mode
- ✅ Comprehensive test suite
- ✅ RPC configuration in UI

### What Needs Fixing:
- ❌ Arduino port detection patterns for macOS
- ❌ WebView keyboard input handling (copy from fork)
- ❌ Proper RPC server startup/integration

## COMPREHENSIVE MIGRATION STRATEGY

### Phase 1: Clean Migration Foundation (PRIORITY 1)
**Objective:** Start fresh with komari_fork, add only the good changes from master

**Steps:**
1. **Preserve komari_fork as baseline** - it has working code
2. **Copy good macOS platform code** from komari-master/platforms/src/macos/
3. **Copy improved Arduino script** with fixes from komari-master
4. **Copy test infrastructure** (test_rpc_client.py)
5. **Update Cargo.toml** with macOS dependencies

**Critical Rule:** DO NOT copy the complex async keyboard handling from master

### Phase 2: Fix Arduino Port Detection (PRIORITY 2)
**Issue:** Arduino script looks for `/dev/tty.*` but you have `/dev/cu.usbmodemHIDFG1`

**Fix:**
```python
# In arduino_example.py, update port detection:
cu_hid_ports = glob.glob('/dev/cu.usbmodem*HID*')  # Your Arduino
tty_hid_ports = glob.glob('/dev/tty.usbmodem*HID*')
cu_other_ports = glob.glob('/dev/cu.usbmodem*') + glob.glob('/dev/cu.usbserial*')
tty_other_ports = glob.glob('/dev/tty.usbmodem*') + glob.glob('/dev/tty.usbserial*')

# Prefer cu.* then tty.*, prefer HID devices
all_ports = cu_hid_ports + tty_hid_ports + cu_other_ports + tty_other_ports
```

### Phase 3: Integration Testing (PRIORITY 3)
**Use the test_rpc_client.py to verify:**
1. Arduino Python server starts correctly
2. Detects your `/dev/cu.usbmodemHIDFG1` device  
3. Komari backend can connect via gRPC
4. End-to-end key commands work

### Phase 4: Documentation and Clean-up
1. Document working setup steps
2. Create startup scripts for Arduino RPC server
3. Clean up any remaining Windows-only code paths

## DETAILED MIGRATION EXECUTION PLAN

### Step 1: Backup and Fresh Start
```bash
# Backup current work
cp -r komari-master komari-master-backup

# Start with clean fork
rm -rf komari-master/*
cp -r komari_fork/* komari-master/
```

### Step 2: Selective Integration  
Copy only these specific good changes from backup:

**From komari-master-backup to komari-master:**
- `platforms/src/macos/` (entire directory)
- `platforms/src/lib.rs` (macOS conditional compilation)
- `backend/src/bridge.rs` (macOS platform imports)
- `platforms/Cargo.toml` (macOS dependencies)
- `examples/python/arduino_example.py` (improved version)
- `examples/python/test_rpc_client.py` (new test file)

**DO NOT COPY:**
- `ui/src/inputs/keys.rs` (keep original simple version)

### Step 3: Apply Arduino Port Fix
Edit the copied `arduino_example.py` to fix port detection patterns.

### Step 4: Test and Validate
1. Run `test_rpc_client.py` 
2. Start Arduino RPC server
3. Test Komari backend connection
4. Verify keyboard input works without WebView crashes

## SUCCESS CRITERIA
1. ✅ Arduino RPC server detects `/dev/cu.usbmodemHIDFG1`
2. ✅ Komari connects to Arduino via gRPC  
3. ✅ All 76 keys work (use test client to verify)
4. ✅ WebView UI works without crashes
5. ✅ macOS screen capture works at coordinates (1770, 270)

## ESTIMATED TIMELINE
- Step 1-2: 15 minutes (backup and selective copy)
- Step 3: 5 minutes (fix Arduino port detection)
- Step 4: 10 minutes (testing and validation)

**Total: 30 minutes to working system**

## RISK MITIGATION
- Keep komari_fork untouched as fallback
- Test each step incrementally
- Use test_rpc_client.py to validate each component
- Document any issues encountered for future reference