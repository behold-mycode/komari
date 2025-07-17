use std::collections::HashMap;
use std::fmt::Debug;
use std::{any::Any, cell::RefCell};

use anyhow::Result;
#[cfg(test)]
use mockall::automock;
#[cfg(windows)]
use platforms::windows::{
    self, BitBltCapture, Frame, Handle, KeyInputKind, KeyKind, Keys, WgcCapture, WindowBoxCapture,
};

#[cfg(target_os = "macos")]
use platforms::macos::{
    self, BitBltCapture, Frame, Handle, KeyKind, KeyInputKind, Keys, screenshot::ScreenshotCapture,
};

use crate::context::MS_PER_TICK_F32;
use crate::database::Seeds;
use crate::rng::Rng;
use crate::rpc;
use crate::{CaptureMode, context::MS_PER_TICK, rpc::KeysService, database::Settings};

/// Base mean in milliseconds to generate a pair from.
const BASE_MEAN_MS_DELAY: f32 = 100.0;

/// Base standard deviation in milliseconds to generate a pair from.
const BASE_STD_MS_DELAY: f32 = 20.0;

/// The rate at which generated standard deviation will revert to the base [`BASE_STD_MS_DELAY`]
/// over time.
const MEAN_STD_REVERSION_RATE: f32 = 0.2;

/// The rate at which generated mean will revert to the base [`BASE_MEAN_MS_DELAY`] over time.
const MEAN_STD_VOLATILITY: f32 = 3.0;

/// The input method to use for the key sender.
///
/// This is a bridge enum between platform-specific and gRPC input options.
pub enum KeySenderMethod {
    Rpc(Handle, String),
    Default(Handle, KeyInputKind),
}

/// The inner kind of the key sender.
///
/// The above [`KeySenderMethod`] will be converted to this inner kind that contains the actual
/// sending structure.
#[derive(Debug)]
enum KeySenderKind {
    Rpc(Handle, Option<RefCell<KeysService>>),
    Default(Keys),
}

#[derive(Debug)]
pub enum MouseAction {
    Move,
    Click,
    Scroll,
}

/// A trait for sending keys.
#[cfg_attr(test, automock)]
pub trait KeySender: Debug {
    fn set_method(&mut self, method: KeySenderMethod);

    fn send(&self, kind: KeyKind) -> Result<()>;

    /// Sends mouse to `(x, y)` relative to the client coordinate (e.g. capture area) and
    /// perform an action.
    ///
    /// `(0, 0)` is top-left and `(width, height)` is bottom-right.
    ///
    /// TODO: Unfortunate name and location...
    fn send_mouse(&self, x: i32, y: i32, action: MouseAction) -> Result<()>;

    fn send_up(&self, kind: KeyKind) -> Result<()>;

    fn send_down(&self, kind: KeyKind) -> Result<()>;

    fn all_keys_cleared(&self) -> bool;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Debug)]
pub struct DefaultKeySender {
    kind: KeySenderKind,
    delay_rng: Rng,
    delay_mean_std_pair: (f32, f32),
    delay_map: RefCell<HashMap<KeyKind, u32>>,
}

#[derive(Debug)]
enum InputDelay {
    Untracked,
    Tracked,
    AlreadyTracked,
}

impl DefaultKeySender {
    pub fn new(method: KeySenderMethod, seeds: Seeds) -> Self {
        Self {
            kind: to_key_sender_kind_from(method, &seeds.seed),
            delay_rng: Rng::new(seeds.seed),
            delay_mean_std_pair: (BASE_MEAN_MS_DELAY, BASE_STD_MS_DELAY),
            delay_map: RefCell::new(HashMap::new()),
        }
    }

    #[inline]
    fn send_inner(&self, kind: KeyKind) -> Result<()> {
        match &self.kind {
            KeySenderKind::Rpc(_, service) => {
                if let Some(cell) = service {
                    cell.borrow_mut()
                        .send(kind, self.random_input_delay_tick_count().0)?;
                }
                Ok(())
            }
            KeySenderKind::Default(keys) => {
                match self.track_input_delay(kind) {
                    InputDelay::Untracked => keys.send(kind)?,
                    InputDelay::Tracked => keys.send_down(kind)?,
                    InputDelay::AlreadyTracked => (),
                }
                Ok(())
            }
        }
    }

    #[inline]
    fn send_up_inner(&self, kind: KeyKind, forced: bool) -> Result<()> {
        match &self.kind {
            KeySenderKind::Rpc(_, service) => {
                if let Some(cell) = service {
                    cell.borrow_mut().send_up(kind)?;
                }
                Ok(())
            }
            KeySenderKind::Default(keys) => {
                if forced || !self.has_input_delay(kind) {
                    keys.send_up(kind)?;
                }
                Ok(())
            }
        }
    }

    #[inline]
    fn send_down_inner(&self, kind: KeyKind) -> Result<()> {
        match &self.kind {
            KeySenderKind::Rpc(_, service) => {
                if let Some(cell) = service {
                    cell.borrow_mut().send_down(kind)?;
                }
                Ok(())
            }
            KeySenderKind::Default(keys) => {
                if !self.has_input_delay(kind) {
                    keys.send_down(kind)?;
                }
                Ok(())
            }
        }
    }

    #[inline]
    fn has_input_delay(&self, kind: KeyKind) -> bool {
        self.delay_map.borrow().contains_key(&kind)
    }

    /// Tracks input delay for a key that is about to be pressed for both down and up key strokes.
    ///
    /// Upon returning [`InputDelay::Tracked`], it is expected that only key down is sent. Later,
    /// it will be automatically released by [`Self::update_input_delay`] once the input delay has
    /// timed out. If [`InputDelay::Untracked`] is returned, it is expected that both down and up
    /// key strokes are sent.
    ///
    /// This function should only be used for [`Self::send`] as the other two should be handled
    /// by the external caller.
    fn track_input_delay(&self, kind: KeyKind) -> InputDelay {
        let mut map = self.delay_map.borrow_mut();
        if map.contains_key(&kind) {
            return InputDelay::AlreadyTracked;
        }

        let (_, delay_tick_count) = self.random_input_delay_tick_count();
        if delay_tick_count > 0 {
            let _ = map.insert(kind, delay_tick_count);
            InputDelay::Tracked
        } else {
            InputDelay::Untracked
        }
    }

    /// Updates the input delay (key up timing) for held down keys and delay std/mean pair.
    #[inline]
    pub fn update_input_delay(&mut self, game_tick: u64) {
        const UPDATE_MEAN_STD_PAIR_INTERVAL: u64 = 200;

        if game_tick > 0 && game_tick.is_multiple_of(UPDATE_MEAN_STD_PAIR_INTERVAL) {
            let (mean, std) = self.delay_mean_std_pair;
            self.delay_mean_std_pair = self.delay_rng.random_mean_std_pair(
                BASE_MEAN_MS_DELAY,
                mean,
                BASE_STD_MS_DELAY,
                std,
                MEAN_STD_REVERSION_RATE,
                MEAN_STD_VOLATILITY,
            )
        }

        let mut map = self.delay_map.borrow_mut();
        if map.is_empty() {
            return;
        }
        map.retain(|kind, delay| {
            *delay = delay.saturating_sub(1);
            if *delay == 0 {
                let _ = self.send_up_inner(*kind, true);
            }
            *delay != 0
        });
    }

    fn random_input_delay_tick_count(&self) -> (f32, u32) {
        let (mean, std) = self.delay_mean_std_pair;
        self.delay_rng
            .random_delay_tick_count(mean, std, MS_PER_TICK_F32, 80.0, 120.0)
    }
}

impl KeySender for DefaultKeySender {
    fn set_method(&mut self, method: KeySenderMethod) {
        match &method {
            KeySenderMethod::Rpc(handle, url) => {
                if let KeySenderKind::Rpc(ref cur_handle, ref option) = self.kind {
                    let service = option.as_ref();
                    let service_borrow = service.map(|service| service.borrow_mut());
                    if let Some(mut borrow) = service_borrow
                        && borrow.url() == url
                        && handle == cur_handle
                    {
                        let _ = borrow.init(self.delay_rng.seed());
                        borrow.reset();
                        return;
                    }
                }
            }
            KeySenderMethod::Default(_, _) => (),
        }
        self.kind = to_key_sender_kind_from(method, self.delay_rng.seed());
    }

    fn send(&self, kind: KeyKind) -> Result<()> {
        self.send_inner(kind)
    }

    fn send_mouse(&self, x: i32, y: i32, action: MouseAction) -> Result<()> {
        match &self.kind {
            KeySenderKind::Rpc(handle, service) => {
                if let Some(cell) = service {
                    let mut borrow = cell.borrow_mut();
                    let coordinates = {
                        #[cfg(windows)]
                        { windows::client_to_monitor_or_frame(*handle, x, y, matches!(borrow.mouse_coordinate(), rpc::Coordinate::Screen))? }
                        #[cfg(target_os = "macos")]
                        { macos::client_to_monitor_or_frame(*handle, x, y, matches!(borrow.mouse_coordinate(), rpc::Coordinate::Screen))? }
                    };
                    let action = match action {
                        MouseAction::Move => rpc::MouseAction::Move,
                        MouseAction::Click => rpc::MouseAction::Click,
                        MouseAction::Scroll => rpc::MouseAction::ScrollDown,
                    };

                    borrow.send_mouse(
                        coordinates.width,
                        coordinates.height,
                        coordinates.x,
                        coordinates.y,
                        action,
                    )?;
                }
                Ok(())
            }
            KeySenderKind::Default(keys) => {
                let action = {
                    #[cfg(windows)]
                    {
                        match action {
                            MouseAction::Move => windows::MouseAction::Move,
                            MouseAction::Click => windows::MouseAction::Click,
                            MouseAction::Scroll => windows::MouseAction::Scroll,
                        }
                    }
                    #[cfg(target_os = "macos")]
                    {
                        match action {
                            MouseAction::Move => macos::MouseAction::Move,
                            MouseAction::Click => macos::MouseAction::Click,
                            MouseAction::Scroll => macos::MouseAction::Scroll,
                        }
                    }
                };
                keys.send_mouse(x, y, action)?;
                Ok(())
            }
        }
    }

    fn send_up(&self, kind: KeyKind) -> Result<()> {
        self.send_up_inner(kind, false)
    }

    fn send_down(&self, kind: KeyKind) -> Result<()> {
        self.send_down_inner(kind)
    }

    #[inline]
    fn all_keys_cleared(&self) -> bool {
        self.delay_map.borrow().is_empty()
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A bridge enum between platform-specific and database capture options.
#[derive(Debug)]
pub enum ImageCaptureKind {
    BitBlt(BitBltCapture),
    #[cfg(windows)]
    Wgc(Option<WgcCapture>),
    #[cfg(windows)]
    BitBltArea(WindowBoxCapture),
    #[cfg(target_os = "macos")]
    BitBltArea(ScreenshotCapture),
    #[cfg(target_os = "macos")]
    Screenshot(ScreenshotCapture),
}

/// A struct for managing different capture modes.
#[derive(Debug)]
pub struct ImageCapture {
    kind: ImageCaptureKind,
}

impl ImageCapture {
    pub fn new(handle: Handle, mode: CaptureMode, settings: &Settings) -> Self {
        Self {
            kind: to_image_capture_kind_from(handle, mode, settings),
        }
    }

    pub fn kind(&self) -> &ImageCaptureKind {
        &self.kind
    }

    pub fn grab(&mut self) -> Option<Frame> {
        match &mut self.kind {
            ImageCaptureKind::BitBlt(capture) => capture.grab().ok(),
            #[cfg(windows)]
            ImageCaptureKind::Wgc(capture) => {
                capture.as_mut().and_then(|capture| capture.grab().ok())
            }
            #[cfg(windows)]
            ImageCaptureKind::BitBltArea(capture) => capture.grab().ok(),
            #[cfg(target_os = "macos")]
            ImageCaptureKind::BitBltArea(capture) => capture.grab().ok(),
            #[cfg(target_os = "macos")]
            ImageCaptureKind::Screenshot(capture) => capture.grab().ok(),
        }
    }

    pub fn set_mode(&mut self, handle: Handle, mode: CaptureMode, settings: &Settings) {
        self.kind = to_image_capture_kind_from(handle, mode, settings);
    }
}

#[inline]
fn to_key_sender_kind_from(method: KeySenderMethod, seed: &[u8]) -> KeySenderKind {
    match method {
        KeySenderMethod::Rpc(handle, url) => {
            let mut service = KeysService::connect(url);
            if let Ok(ref mut service) = service {
                let _ = service.init(seed);
            }
            KeySenderKind::Rpc(handle, service.ok().map(RefCell::new))
        }
        KeySenderMethod::Default(handle, kind) => KeySenderKind::Default(Keys::new(handle, kind)),
    }
}

#[inline]
fn to_image_capture_kind_from(handle: Handle, mode: CaptureMode, settings: &Settings) -> ImageCaptureKind {
    match mode {
        #[cfg(windows)]
        CaptureMode::BitBlt => ImageCaptureKind::BitBlt(BitBltCapture::new(handle, false)),
        #[cfg(target_os = "macos")]
        CaptureMode::BitBlt => {
            match BitBltCapture::new(handle) {
                Ok(capture) => ImageCaptureKind::BitBlt(capture),
                Err(e) => {
                    log::warn!("Failed to create BitBltCapture with handle coordinates: {:?}", e);
                    log::info!("Falling back to safe default coordinates (0, 0, 1366, 768)");
                    let fallback_handle = handle.with_coordinates(0, 0, 0, 1366, 768);
                    match BitBltCapture::new(fallback_handle) {
                        Ok(capture) => ImageCaptureKind::BitBlt(capture),
                        Err(fallback_e) => {
                            log::error!("Fallback BitBltCapture also failed: {:?}", fallback_e);
                            // Return a minimal capture that won't crash the backend
                            match BitBltCapture::new_with_coordinates(0, 0, 0, 1366, 768) {
                                Ok(capture) => ImageCaptureKind::BitBlt(capture),
                                Err(coord_e) => {
                                    log::error!("Failed to create BitBltCapture with coordinates: {:?}", coord_e);
                                    panic!("Cannot create any BitBlt capture - display system may be unavailable")
                                }
                            }
                        }
                    }
                }
            }
        }
        #[cfg(windows)]
        CaptureMode::WindowsGraphicsCapture => {
            ImageCaptureKind::Wgc(WgcCapture::new(handle, MS_PER_TICK).ok())
        }
        #[cfg(target_os = "macos")]
        CaptureMode::WindowsGraphicsCapture => {
            // Map Windows Graphics Capture to macOS Screenshot API
            match ScreenshotCapture::new(handle) {
                Ok(capture) => ImageCaptureKind::Screenshot(capture),
                Err(e) => {
                    log::warn!("Failed to create screenshot capture: {:?}, using default screen region", e);
                    // Create a handle with safe default coordinates (smaller region to avoid screen bounds issues)
                    let safe_handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1280, 720);
                    match ScreenshotCapture::new(safe_handle) {
                        Ok(capture) => ImageCaptureKind::Screenshot(capture),
                        Err(safe_e) => {
                            log::error!("Failed to create screenshot capture even with default coordinates: {:?}", safe_e);
                            panic!("Cannot create any screenshot capture - display system may be unavailable")
                        }
                    }
                }
            }
        }
        #[cfg(windows)]
        CaptureMode::BitBltArea => ImageCaptureKind::BitBltArea(WindowBoxCapture::default()),
        #[cfg(target_os = "macos")]
        CaptureMode::BitBltArea => {
            // Use coordinates from settings for BitBltArea mode
            let configured_handle = handle.with_coordinates(
                0, // display_index - default to primary display
                settings.capture_x,
                settings.capture_y, 
                1366, // Fixed MapleStory window width
                768   // Fixed MapleStory window height
            );
            
            match ScreenshotCapture::new(configured_handle) {
                Ok(capture) => ImageCaptureKind::BitBltArea(capture),
                Err(e) => {
                    log::warn!("Failed to create screenshot capture for BitBltArea with coordinates ({}, {}): {:?}", 
                              settings.capture_x, settings.capture_y, e);
                    log::info!("Falling back to safe default coordinates (0, 0, 1280, 720)");
                    // Create a handle with safe default coordinates (smaller region to avoid screen bounds issues)
                    let safe_handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1280, 720);
                    match ScreenshotCapture::new(safe_handle) {
                        Ok(capture) => ImageCaptureKind::BitBltArea(capture),
                        Err(fallback_e) => {
                            log::error!("Fallback screenshot capture also failed: {:?}", fallback_e);
                            log::info!("Creating minimal capture that won't crash the backend");
                            // Return a minimal capture that won't crash the backend
                            match ScreenshotCapture::new(
                                Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 800, 600)
                            ) {
                                Ok(capture) => ImageCaptureKind::BitBltArea(capture),
                                Err(minimal_e) => {
                                    log::error!("All screenshot capture attempts failed: {:?}", minimal_e);
                                    log::error!("This may indicate a serious display or permissions issue");
                                    // Return the last working capture or create a dummy one
                                    ImageCaptureKind::BitBltArea(ScreenshotCapture::new(
                                        Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 100, 100)
                                    ).unwrap_or_else(|_| {
                                        panic!("Complete failure: cannot create any screenshot capture")
                                    }))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    const SEED: [u8; 32] = [
        64, 241, 206, 219, 49, 21, 218, 145, 254, 152, 68, 176, 242, 238, 152, 14, 176, 241, 153,
        64, 44, 192, 172, 191, 191, 157, 107, 206, 193, 55, 115, 68,
    ];

    fn test_key_sender() -> DefaultKeySender {
        let seeds = Seeds {
            id: None,
            seed: SEED,
        };
        DefaultKeySender::new(
            KeySenderMethod::Default(Handle::new("Handle"), KeyInputKind::Fixed),
            seeds,
        )
    }

    #[test]
    fn track_input_delay_tracked() {
        let sender = test_key_sender();

        // Force rng to generate delay > 0
        let result = sender.track_input_delay(KeyKind::Ctrl);
        assert_matches!(result, InputDelay::Tracked);
        assert!(sender.has_input_delay(KeyKind::Ctrl));
    }

    #[test]
    fn track_input_delay_already_tracked() {
        let sender = test_key_sender();
        sender.delay_map.borrow_mut().insert(KeyKind::Ctrl, 3);

        let result = sender.track_input_delay(KeyKind::Ctrl);
        assert_matches!(result, InputDelay::AlreadyTracked);
    }

    #[test]
    fn update_input_delay_decrement_and_release_key() {
        let mut sender = test_key_sender();
        let count = 50;
        sender.delay_map.borrow_mut().insert(KeyKind::Ctrl, count);

        for _ in 0..count {
            sender.update_input_delay(0);
        }
        // After `count` updates, key should be released and removed
        assert!(!sender.has_input_delay(KeyKind::Ctrl));
    }

    #[test]
    fn update_input_delay_refresh_mean_std_pair_every_interval() {
        let mut sender = test_key_sender();
        let original_pair = sender.delay_mean_std_pair;

        // Simulate tick before the interval: should NOT update
        sender.update_input_delay(199);
        assert_eq!(sender.delay_mean_std_pair, original_pair);

        // Simulate tick AT the interval: should update
        sender.update_input_delay(200);
        assert_ne!(sender.delay_mean_std_pair, original_pair);
    }
}
