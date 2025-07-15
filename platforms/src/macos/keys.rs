use super::{Error, Handle};

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

#[derive(Debug)]
pub struct KeysManager {
    handle: super::handle::HandleCell,
    key_input_kind: KeyInputKind,
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
        Self {
            handle: super::handle::HandleCell::new(handle),
            key_input_kind: kind,
        }
    }

    pub fn send(&self, key: KeyKind) -> Result<(), Error> {
        self.send_down(key)?;
        self.send_up(key)
    }

    pub fn send_down(&self, key: KeyKind) -> Result<(), Error> {
        // TODO: Implement macOS key down via Arduino RPC or Enigo fallback
        Ok(())
    }

    pub fn send_up(&self, key: KeyKind) -> Result<(), Error> {
        // TODO: Implement macOS key up via Arduino RPC or Enigo fallback
        Ok(())
    }

    pub fn send_mouse(&self, x: i32, y: i32, action: MouseAction) -> Result<(), Error> {
        // TODO: Implement macOS mouse actions via Arduino RPC or Enigo fallback
        Ok(())
    }
}

#[derive(Debug)]
pub enum MouseAction {
    Move,
    Click,
    Scroll,
}

use std::sync::{Mutex, OnceLock, mpsc::{self, Receiver}};

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
        let (_tx, rx) = mpsc::channel();
        Self {
            handle: super::handle::HandleCell::new(handle),
            key_input_kind,
            rx,
        }
    }

    pub fn try_recv(&mut self) -> Option<KeyKind> {
        self.rx.try_recv().ok()
    }
}

pub fn run_event_loop() {
    // TODO: Implement macOS event loop for input handling
    // This will integrate with Arduino RPC system or Enigo fallback
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
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