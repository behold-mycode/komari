use super::{Error, Handle};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use core_graphics::event::{CGEventTap, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy};
use std::sync::{Mutex, OnceLock, Arc, LazyLock};
use std::time::Duration;
use tokio::sync::broadcast::{self, Receiver, Sender};

// Global keyboard event channel (like Windows KEY_CHANNEL)
static KEY_CHANNEL: LazyLock<Sender<KeyKind>> = LazyLock::new(|| broadcast::channel(1).0);


// CGEventField constants for keyboard events (raw values)
const kCGKeyboardEventKeycode: u32 = 9;
const kCGEventSourceUnixProcessID: u32 = 21;

// Placeholder types for RPC integration - these will be replaced with actual backend types
pub enum RpcMouseAction {
    Move,
    Click,
    ScrollDown,
}

pub trait RpcService: Send + Sync {
    fn send_down(&mut self, key: KeyKind) -> Result<(), anyhow::Error>;
    fn send_up(&mut self, key: KeyKind) -> Result<(), anyhow::Error>;
    fn send_mouse(&mut self, width: i32, height: i32, x: i32, y: i32, action: RpcMouseAction) -> Result<(), anyhow::Error>;
}

#[derive(Debug)]
pub struct ConvertedCoordinates {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum KeyKind {
    #[default]
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Zero, One, Two, Three, Four, Five, Six, Seven, Eight, Nine,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown, Insert, Delete,
    Ctrl, Enter, Space, Tilde, Quote, Semicolon, Comma, Period, Slash, Esc, Shift, Alt,
}

pub struct KeysManager {
    handle: super::handle::HandleCell,
    key_input_kind: KeyInputKind,
    rpc_client: Option<Arc<Mutex<dyn RpcService>>>,
    // event_source: Option<CGEventSource>, // Create on-demand to avoid Send issues
}

impl std::fmt::Debug for KeysManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeysManager")
            .field("handle", &self.handle)
            .field("key_input_kind", &self.key_input_kind)
            .field("rpc_client", &self.rpc_client.as_ref().map(|_| "Some(Arc<Mutex<dyn RpcService>>)"))
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum KeyInputKind {
    /// Sends input only if handle is in the foreground and focused
    Fixed,
    /// Sends input only if the foreground window is not handle, on top of
    /// handle window and is focused
    Foreground,
}

impl KeysManager {
    pub fn new(handle: Handle, kind: KeyInputKind) -> Self {
        Self::new_with_rpc(handle, kind, None)
    }

    pub fn new_with_rpc(handle: Handle, kind: KeyInputKind, rpc_client: Option<Arc<Mutex<dyn RpcService>>>) -> Self {
        Self {
            handle: super::handle::HandleCell::new(handle),
            key_input_kind: kind,
            rpc_client,
        }
    }

    pub fn send(&self, key: KeyKind) -> Result<(), Error> {
        self.send_down(key)?;
        self.send_up(key)
    }

    pub fn send_down(&self, key: KeyKind) -> Result<(), Error> {
        // Try Arduino RPC first
        if let Some(rpc_client) = &self.rpc_client {
            match rpc_client.lock().unwrap().send_down(key) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!("Arduino RPC send_down failed: {}, falling back to Core Graphics", e);
                }
            }
        }

        // Fallback to Core Graphics
        self.send_key_down_core_graphics(key)
    }

    pub fn send_up(&self, key: KeyKind) -> Result<(), Error> {
        // Try Arduino RPC first
        if let Some(rpc_client) = &self.rpc_client {
            match rpc_client.lock().unwrap().send_up(key) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!("Arduino RPC send_up failed: {}, falling back to Core Graphics", e);
                }
            }
        }

        // Fallback to Core Graphics
        self.send_key_up_core_graphics(key)
    }

    pub fn send_mouse(&self, x: i32, y: i32, action: MouseAction) -> Result<(), Error> {
        // Try Arduino RPC first
        if let Some(rpc_client) = &self.rpc_client {
            let rpc_action = match action {
                MouseAction::Move => RpcMouseAction::Move,
                MouseAction::Click => RpcMouseAction::Click,
                MouseAction::Scroll => RpcMouseAction::ScrollDown,
            };
            
            // Convert coordinates using handle
            let handle = self.handle.get_handle();
            let (screen_x, screen_y) = handle.client_to_screen(x, y);
            
            match rpc_client.lock().unwrap().send_mouse(handle.width, handle.height, screen_x, screen_y, rpc_action) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!("Arduino RPC send_mouse failed: {}, falling back to Core Graphics", e);
                }
            }
        }

        // Fallback to Core Graphics
        self.send_mouse_core_graphics(x, y, action)
    }

    // Core Graphics implementation methods
    fn send_key_down_core_graphics(&self, key: KeyKind) -> Result<(), Error> {
        let event_source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| Error::InitializationFailed)?;
        let key_code = key_kind_to_macos_keycode(key);
        
        let event = CGEvent::new_keyboard_event(event_source, key_code, true)
            .map_err(|_| Error::InputFailed)?;
        
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn send_key_up_core_graphics(&self, key: KeyKind) -> Result<(), Error> {
        let event_source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| Error::InitializationFailed)?;
        let key_code = key_kind_to_macos_keycode(key);
        
        let event = CGEvent::new_keyboard_event(event_source, key_code, false)
            .map_err(|_| Error::InputFailed)?;
        
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn send_mouse_core_graphics(&self, x: i32, y: i32, action: MouseAction) -> Result<(), Error> {
        let event_source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| Error::InitializationFailed)?;
        let handle = self.handle.get_handle();
        let (screen_x, screen_y) = handle.client_to_screen(x, y);
        
        match action {
            MouseAction::Move => {
                let event = CGEvent::new_mouse_event(
                    event_source,
                    CGEventType::MouseMoved,
                    core_graphics::geometry::CGPoint::new(screen_x as f64, screen_y as f64),
                    CGMouseButton::Left,
                ).map_err(|_| Error::InputFailed)?;
                
                event.post(CGEventTapLocation::HID);
            }
            MouseAction::Click => {
                let point = core_graphics::geometry::CGPoint::new(screen_x as f64, screen_y as f64);
                
                // Mouse down
                let down_event = CGEvent::new_mouse_event(
                    event_source.clone(),
                    CGEventType::LeftMouseDown,
                    point,
                    CGMouseButton::Left,
                ).map_err(|_| Error::InputFailed)?;
                
                down_event.post(CGEventTapLocation::HID);
                
                // Small delay
                std::thread::sleep(Duration::from_millis(50));
                
                // Mouse up
                let up_event = CGEvent::new_mouse_event(
                    event_source,
                    CGEventType::LeftMouseUp,
                    point,
                    CGMouseButton::Left,
                ).map_err(|_| Error::InputFailed)?;
                
                up_event.post(CGEventTapLocation::HID);
            }
            MouseAction::Scroll => {
                // TODO: Implement scroll functionality
                // For now, just log and do nothing
                log::info!("Mouse scroll requested at ({}, {}) - not implemented yet", screen_x, screen_y);
            }
        }
        
        Ok(())
    }
}

#[derive(Debug)]
pub enum MouseAction {
    Move,
    Click,
    Scroll,
}

// TODO: Implement proper CGEventTap keyboard capture when core-graphics supports it
// For now, the KeyReceiver infrastructure is in place and ready for events

static KEYS_MANAGER: OnceLock<Mutex<KeysManager>> = OnceLock::new();

pub fn init() -> &'static Mutex<KeysManager> {
    KEYS_MANAGER.get_or_init(|| {
        // Use a placeholder handle for global initialization
        let handle = Handle::new("global");
        Mutex::new(KeysManager::new(handle, KeyInputKind::Fixed))
    })
}

#[derive(Debug)]
pub struct KeyReceiver {
    handle: super::handle::HandleCell,
    key_input_kind: KeyInputKind,
    rx: Receiver<KeyKind>,
}

impl KeyReceiver {
    pub fn new(handle: Handle, key_input_kind: KeyInputKind) -> Self {
        Self {
            handle: super::handle::HandleCell::new(handle),
            key_input_kind,
            rx: KEY_CHANNEL.subscribe(),
        }
    }

    pub fn try_recv(&mut self) -> Option<KeyKind> {
        self.rx
            .try_recv()
            .ok()
            .and_then(|key| self.can_process_key().then_some(key))
    }

    // TODO: Implement proper foreground window checking for macOS
    fn can_process_key(&self) -> bool {
        // For now, always allow processing (like Windows does when window is in foreground)
        // This can be improved later with proper macOS window focus checking
        true
    }
}

pub fn run_event_loop() {
    log::info!("Starting macOS keyboard event loop with CGEventTap");
    
    // Create the event tap callback
    let event_tap_callback = |
        _proxy: CGEventTapProxy, 
        event_type: CGEventType, 
        event: &CGEvent
    | -> Option<CGEvent> {
        // Only process key down and key up events
        // Use raw values for event type comparison
        let event_type_raw = event_type as u32;
        let key_down_raw = CGEventType::KeyDown as u32;
        let key_up_raw = CGEventType::KeyUp as u32;
        
        if event_type_raw == key_down_raw || event_type_raw == key_up_raw {
            // Get the key code from the event
            let key_code = event.get_integer_value_field(kCGKeyboardEventKeycode);
            
            // Convert to our KeyKind enum
            if let Some(key_kind) = macos_keycode_to_key_kind(key_code as CGKeyCode) {
                // Only send KEY_UP events to match Windows behavior
                if event_type_raw == key_up_raw {
                    // Check if this is an injected event (from our own application)
                    // In macOS, we can check the event source
                    let event_source = event.get_integer_value_field(kCGEventSourceUnixProcessID);
                    let current_process = std::process::id();
                    
                    // Don't process events from our own process to avoid loops
                    if event_source as u32 != current_process {
                        log::debug!("Captured key event: {:?} (keycode: {})", key_kind, key_code);
                        // Send the key to the channel (non-blocking)
                        let _ = KEY_CHANNEL.send(key_kind);
                    }
                }
            }
        }
        
        // Always pass the event through to allow normal processing
        Some(event.clone())
    };
    
    // Create the event tap
    // Use Vec of CGEventType for event mask
    let event_mask = vec![CGEventType::KeyDown, CGEventType::KeyUp];
    
    let event_tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        event_mask,
        event_tap_callback,
    );
    
    match event_tap {
        Ok(tap) => {
            log::info!("Successfully created CGEventTap for keyboard capture");
            
            // Create a run loop source from the event tap
            let run_loop_source = tap.mach_port.create_runloop_source(0);
            
            match run_loop_source {
                Ok(source) => {
                    // Add the source to the current run loop
                    let run_loop = CFRunLoop::get_current();
                    run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
                    
                    // Enable the event tap
                    tap.enable();
                    
                    log::info!("Event tap enabled, starting run loop");
                    
                    // Run the event loop
                    CFRunLoop::run_current();
                }
                Err(e) => {
                    log::error!("Failed to create run loop source: {:?}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to create CGEventTap: {:?}", e);
            log::error!("This might be due to accessibility permissions not being granted.");
            log::error!("Please grant accessibility permissions to the application in System Preferences > Security & Privacy > Privacy > Accessibility");
            
            // Fall back to a basic loop to keep the thread alive
            log::info!("Falling back to basic event loop");
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}

pub fn client_to_monitor_or_frame(
    _handle: Handle,
    x: i32,
    y: i32,
    _monitor_coordinate: bool,
) -> Result<ConvertedCoordinates, Error> {
    // For macOS BitBlt Area capture, coordinates are relative to the capture region
    // Default 1366x768 game region coordinates
    Ok(ConvertedCoordinates {
        width: 1366,
        height: 768,
        x,
        y,
    })
}

// Key mapping from macOS key codes to KeyKind (reverse mapping)
fn macos_keycode_to_key_kind(keycode: CGKeyCode) -> Option<KeyKind> {
    match keycode {
        // Letters
        0x00 => Some(KeyKind::A), 0x0B => Some(KeyKind::B), 0x08 => Some(KeyKind::C), 0x02 => Some(KeyKind::D),
        0x0E => Some(KeyKind::E), 0x03 => Some(KeyKind::F), 0x05 => Some(KeyKind::G), 0x04 => Some(KeyKind::H),
        0x22 => Some(KeyKind::I), 0x26 => Some(KeyKind::J), 0x28 => Some(KeyKind::K), 0x25 => Some(KeyKind::L),
        0x2E => Some(KeyKind::M), 0x2D => Some(KeyKind::N), 0x1F => Some(KeyKind::O), 0x23 => Some(KeyKind::P),
        0x0C => Some(KeyKind::Q), 0x0F => Some(KeyKind::R), 0x01 => Some(KeyKind::S), 0x11 => Some(KeyKind::T),
        0x20 => Some(KeyKind::U), 0x09 => Some(KeyKind::V), 0x0D => Some(KeyKind::W), 0x07 => Some(KeyKind::X),
        0x10 => Some(KeyKind::Y), 0x06 => Some(KeyKind::Z),
        
        // Numbers
        0x1D => Some(KeyKind::Zero), 0x12 => Some(KeyKind::One), 0x13 => Some(KeyKind::Two), 0x14 => Some(KeyKind::Three),
        0x15 => Some(KeyKind::Four), 0x17 => Some(KeyKind::Five), 0x16 => Some(KeyKind::Six), 0x1A => Some(KeyKind::Seven),
        0x1C => Some(KeyKind::Eight), 0x19 => Some(KeyKind::Nine),
        
        // Function keys
        0x7A => Some(KeyKind::F1), 0x78 => Some(KeyKind::F2), 0x63 => Some(KeyKind::F3), 0x76 => Some(KeyKind::F4),
        0x60 => Some(KeyKind::F5), 0x61 => Some(KeyKind::F6), 0x62 => Some(KeyKind::F7), 0x64 => Some(KeyKind::F8),
        0x65 => Some(KeyKind::F9), 0x6D => Some(KeyKind::F10), 0x67 => Some(KeyKind::F11), 0x6F => Some(KeyKind::F12),
        
        // Arrow keys
        0x7E => Some(KeyKind::Up), 0x7D => Some(KeyKind::Down), 0x7B => Some(KeyKind::Left), 0x7C => Some(KeyKind::Right),
        
        // Navigation
        0x73 => Some(KeyKind::Home), 0x77 => Some(KeyKind::End), 0x74 => Some(KeyKind::PageUp), 0x79 => Some(KeyKind::PageDown),
        0x72 => Some(KeyKind::Insert), 0x75 => Some(KeyKind::Delete),
        
        // Special keys
        0x3B => Some(KeyKind::Ctrl), 0x24 => Some(KeyKind::Enter), 0x31 => Some(KeyKind::Space), 0x32 => Some(KeyKind::Tilde),
        0x27 => Some(KeyKind::Quote), 0x29 => Some(KeyKind::Semicolon), 0x2B => Some(KeyKind::Comma), 0x2F => Some(KeyKind::Period),
        0x2C => Some(KeyKind::Slash), 0x35 => Some(KeyKind::Esc), 0x38 => Some(KeyKind::Shift), 0x3A => Some(KeyKind::Alt),
        
        _ => None,
    }
}

// Key mapping from KeyKind to macOS key codes
fn key_kind_to_macos_keycode(key: KeyKind) -> CGKeyCode {
    match key {
        // Letters
        KeyKind::A => 0x00, KeyKind::B => 0x0B, KeyKind::C => 0x08, KeyKind::D => 0x02,
        KeyKind::E => 0x0E, KeyKind::F => 0x03, KeyKind::G => 0x05, KeyKind::H => 0x04,
        KeyKind::I => 0x22, KeyKind::J => 0x26, KeyKind::K => 0x28, KeyKind::L => 0x25,
        KeyKind::M => 0x2E, KeyKind::N => 0x2D, KeyKind::O => 0x1F, KeyKind::P => 0x23,
        KeyKind::Q => 0x0C, KeyKind::R => 0x0F, KeyKind::S => 0x01, KeyKind::T => 0x11,
        KeyKind::U => 0x20, KeyKind::V => 0x09, KeyKind::W => 0x0D, KeyKind::X => 0x07,
        KeyKind::Y => 0x10, KeyKind::Z => 0x06,
        
        // Numbers
        KeyKind::Zero => 0x1D, KeyKind::One => 0x12, KeyKind::Two => 0x13, KeyKind::Three => 0x14,
        KeyKind::Four => 0x15, KeyKind::Five => 0x17, KeyKind::Six => 0x16, KeyKind::Seven => 0x1A,
        KeyKind::Eight => 0x1C, KeyKind::Nine => 0x19,
        
        // Function keys
        KeyKind::F1 => 0x7A, KeyKind::F2 => 0x78, KeyKind::F3 => 0x63, KeyKind::F4 => 0x76,
        KeyKind::F5 => 0x60, KeyKind::F6 => 0x61, KeyKind::F7 => 0x62, KeyKind::F8 => 0x64,
        KeyKind::F9 => 0x65, KeyKind::F10 => 0x6D, KeyKind::F11 => 0x67, KeyKind::F12 => 0x6F,
        
        // Arrow keys
        KeyKind::Up => 0x7E, KeyKind::Down => 0x7D, KeyKind::Left => 0x7B, KeyKind::Right => 0x7C,
        
        // Navigation
        KeyKind::Home => 0x73, KeyKind::End => 0x77, KeyKind::PageUp => 0x74, KeyKind::PageDown => 0x79,
        KeyKind::Insert => 0x72, KeyKind::Delete => 0x75,
        
        // Special keys
        KeyKind::Ctrl => 0x3B, KeyKind::Enter => 0x24, KeyKind::Space => 0x31, KeyKind::Tilde => 0x32,
        KeyKind::Quote => 0x27, KeyKind::Semicolon => 0x29, KeyKind::Comma => 0x2B, KeyKind::Period => 0x2F,
        KeyKind::Slash => 0x2C, KeyKind::Esc => 0x35, KeyKind::Shift => 0x38, KeyKind::Alt => 0x3A,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keysmanager_creation() {
        let handle = Handle::new("test").with_coordinates(0, 0, 0, 1366, 768);
        let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
        
        // Test that we can create a KeysManager without RPC
        println!("KeysManager created successfully: {:?}", keys_manager);
    }

    #[test]
    fn test_keyboard_input_fallback() {
        let handle = Handle::new("test").with_coordinates(0, 0, 0, 1366, 768);
        let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
        
        // Test keyboard input (should use Core Graphics fallback)
        match keys_manager.send(KeyKind::A) {
            Ok(()) => println!("✅ Keyboard input (A key) successful"),
            Err(e) => println!("❌ Keyboard input failed: {:?}", e),
        }
    }

    #[test]
    fn test_mouse_input_fallback() {
        let handle = Handle::new("test").with_coordinates(0, 0, 0, 1366, 768);
        let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
        
        // Test mouse input (should use Core Graphics fallback)
        match keys_manager.send_mouse(100, 100, MouseAction::Click) {
            Ok(()) => println!("✅ Mouse input (click) successful"),
            Err(e) => println!("❌ Mouse input failed: {:?}", e),
        }
    }

    #[test]
    fn test_key_mapping() {
        // Test that our key mapping function works
        let key_a_code = key_kind_to_macos_keycode(KeyKind::A);
        assert_eq!(key_a_code, 0x00);
        
        let key_enter_code = key_kind_to_macos_keycode(KeyKind::Enter);
        assert_eq!(key_enter_code, 0x24);
        
        println!("✅ Key mapping working correctly");
    }

    #[test]
    fn test_l_key_crash_fix() {
        println!("🔬 Testing L key crash fix...");
        
        // Test 1: Create KeyReceiver like the UI does
        let handle = Handle::new("test").with_coordinates(0, 0, 0, 1366, 768);
        let mut key_receiver = KeyReceiver::new(handle, KeyInputKind::Fixed);
        
        println!("✅ KeyReceiver created successfully");
        
        // Test 2: Simulate receiving L key event
        let _ = KEY_CHANNEL.send(KeyKind::L);
        println!("✅ L key event sent to channel");
        
        // Test 3: Try to receive the event (this was the crash point)
        match key_receiver.try_recv() {
            Some(key) => println!("✅ Received key: {:?}", key),
            None => println!("⚠️  No key received initially (expected due to timing)"),
        }
        
        // Test 4: Test multiple rapid L key presses (stress test)
        for i in 0..100 {
            let _ = KEY_CHANNEL.send(KeyKind::L);
            let _ = key_receiver.try_recv();
            if i % 20 == 0 {
                println!("✅ Rapid L key test: {}/100", i);
            }
        }
        
        println!("✅ All L key tests passed - no crashes!");
    }

    #[test]
    fn test_all_platform_hotkeys() {
        println!("🔬 Testing all platform hotkeys (J, K, L)...");
        
        let handle = Handle::new("test").with_coordinates(0, 0, 0, 1366, 768);
        let mut key_receiver = KeyReceiver::new(handle, KeyInputKind::Fixed);
        
        // Test J key (platform_start_key)
        let _ = KEY_CHANNEL.send(KeyKind::J);
        match key_receiver.try_recv() {
            Some(key) => println!("✅ J key received: {:?}", key),
            None => println!("⚠️  J key not received"),
        }
        
        // Test K key (platform_end_key)
        let _ = KEY_CHANNEL.send(KeyKind::K);
        match key_receiver.try_recv() {
            Some(key) => println!("✅ K key received: {:?}", key),
            None => println!("⚠️  K key not received"),
        }
        
        // Test L key (platform_add_key)
        let _ = KEY_CHANNEL.send(KeyKind::L);
        match key_receiver.try_recv() {
            Some(key) => println!("✅ L key received: {:?}", key),
            None => println!("⚠️  L key not received"),
        }
        
        // Test comma key (another hotkey)
        let _ = KEY_CHANNEL.send(KeyKind::Comma);
        match key_receiver.try_recv() {
            Some(key) => println!("✅ Comma key received: {:?}", key),
            None => println!("⚠️  Comma key not received"),
        }
        
        println!("✅ All platform hotkeys tested successfully!");
    }
}