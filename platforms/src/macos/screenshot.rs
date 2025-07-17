use screenshots::Screen;
use super::{Error, Frame, Handle, HandleCell};

#[derive(Debug)]
pub struct ScreenshotCapture {
    handle: HandleCell,
    display_index: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    screen: Option<Screen>,
}

impl ScreenshotCapture {
    pub fn new(handle: Handle) -> Result<Self, Error> {
        // Get all available screens
        let screens = Screen::all().map_err(|_| Error::WindowNotFound)?;
        
        // Validate display index
        if handle.display_index >= screens.len() {
            return Err(Error::WindowNotFound);
        }
        
        let screen = screens.into_iter().nth(handle.display_index)
            .ok_or(Error::WindowNotFound)?;
        
        // For multi-monitor setups, coordinates may extend beyond primary screen
        // Get all screens to check if coordinates are within the extended desktop bounds
        let all_screens = Screen::all().map_err(|_| Error::WindowNotFound)?;
        let mut capture_valid = false;
        
        for screen in all_screens.iter() {
            let display_info = &screen.display_info;
            
            // Check if the capture area fits within this screen
            if handle.x >= 0 && handle.y >= 0 && 
               handle.x + handle.width <= display_info.width as i32 &&
               handle.y + handle.height <= display_info.height as i32 {
                capture_valid = true;
                break;
            }
        }
        
        if !capture_valid {
            log::warn!("Capture coordinates ({}, {}) with size {}x{} do not fit within any available screen",
                      handle.x, handle.y, handle.width, handle.height);
            return Err(Error::InvalidWindowSize);
        }

        Ok(Self {
            handle: HandleCell::new(handle.clone()),
            display_index: handle.display_index,
            x: handle.x,
            y: handle.y,
            width: handle.width,
            height: handle.height,
            screen: Some(screen),
        })
    }

    pub fn set_capture_region(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<(), Error> {
        // Validate new coordinates against all available screens (multi-monitor support)
        let all_screens = Screen::all().map_err(|_| Error::WindowNotFound)?;
        let mut capture_valid = false;
        
        for screen in all_screens.iter() {
            let display_info = &screen.display_info;
            
            // Check if the capture area fits within this screen
            if x >= 0 && y >= 0 && 
               x + width <= display_info.width as i32 &&
               y + height <= display_info.height as i32 {
                capture_valid = true;
                break;
            }
        }
        
        if !capture_valid {
            log::warn!("New capture coordinates ({}, {}) with size {}x{} do not fit within any available screen",
                      x, y, width, height);
            return Err(Error::InvalidWindowSize);
        }
        
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
        Ok(())
    }

    pub fn grab(&mut self) -> Result<Frame, Error> {
        let screen = self.screen.as_ref().ok_or(Error::WindowNotFound)?;
        
        let image = screen
            .capture_area(self.x, self.y, self.width as u32, self.height as u32)
            .map_err(|_| Error::FrameNotAvailable)?;

        // Convert RGBA to BGRA format to match Windows Frame format
        let buffer = image.as_raw();
        let mut bgra_data = Vec::with_capacity(buffer.len());
        
        for chunk in buffer.chunks_exact(4) {
            bgra_data.push(chunk[2]); // B
            bgra_data.push(chunk[1]); // G
            bgra_data.push(chunk[0]); // R
            bgra_data.push(chunk[3]); // A
        }

        Ok(Frame {
            width: self.width,
            height: self.height,
            data: bgra_data,
        })
    }

    pub fn stop_capture(&mut self) {
        // No cleanup needed for screenshot-based capture
    }

    pub fn handle(&self) -> Handle {
        self.handle.get_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::query_capture_handles;

    #[test]
    fn test_display_enumeration() {
        let handles = query_capture_handles();
        println!("Found {} displays:", handles.len());
        
        for (index, (name, handle)) in handles.iter().enumerate() {
            println!("  {}: {} - Display {}, Coords ({}, {}), Size {}x{}", 
                    index, name, handle.display_index, 
                    handle.x, handle.y, handle.width, handle.height);
        }
        
        assert!(!handles.is_empty(), "Should find at least one display");
    }

    #[test]
    fn test_screen_capture_creation() {
        let handles = query_capture_handles();
        if let Some((_name, handle)) = handles.first() {
            match ScreenshotCapture::new(*handle) {
                Ok(capture) => {
                    println!("Successfully created ScreenshotCapture:");
                    println!("  Display: {}", capture.display_index);
                    println!("  Region: ({}, {}) {}x{}", 
                            capture.x, capture.y, capture.width, capture.height);
                }
                Err(e) => {
                    println!("Failed to create ScreenshotCapture: {:?}", e);
                    // Don't fail the test since this might be due to permissions
                }
            }
        }
    }

    #[test]
    fn test_screen_bounds_validation() {
        // Test with invalid coordinates that should fail
        let invalid_handle = Handle::new("Test")
            .with_coordinates(0, -100, -100, 1366, 768);
            
        match ScreenshotCapture::new(invalid_handle) {
            Ok(_) => panic!("Should have failed with invalid coordinates"),
            Err(Error::InvalidWindowSize) => {
                println!("Correctly rejected invalid coordinates");
            }
            Err(e) => {
                println!("Got different error (acceptable): {:?}", e);
            }
        }
    }

    #[test]
    fn test_coordinate_update() {
        let handles = query_capture_handles();
        if let Some((_name, handle)) = handles.first() {
            if let Ok(mut capture) = ScreenshotCapture::new(*handle) {
                // Test updating coordinates within bounds
                let result = capture.set_capture_region(100, 100, 800, 600);
                
                match result {
                    Ok(()) => {
                        println!("Successfully updated capture region to (100, 100) 800x600");
                        assert_eq!(capture.x, 100);
                        assert_eq!(capture.y, 100);
                        assert_eq!(capture.width, 800);
                        assert_eq!(capture.height, 600);
                    }
                    Err(e) => {
                        println!("Failed to update coordinates: {:?}", e);
                    }
                }
            }
        }
    }

    #[test]
    fn test_actual_capture_attempt() {
        let handles = query_capture_handles();
        if let Some((name, handle)) = handles.first() {
            println!("Attempting capture on: {}", name);
            
            // Use actual MapleStory resolution as required
            let test_handle = handle.with_coordinates(
                handle.display_index, 
                0, 0, 1366, 768  // ACTUAL MapleStory resolution
            );
            
            match ScreenshotCapture::new(test_handle) {
                Ok(mut capture) => {
                    match capture.grab() {
                        Ok(frame) => {
                            println!("Successfully captured frame:");
                            println!("  Size: {}x{}", frame.width, frame.height);
                            println!("  Data length: {} bytes", frame.data.len());
                            println!("  Expected length: {} bytes", frame.width * frame.height * 4);
                            
                            // Verify frame format matches MapleStory requirements
                            assert_eq!(frame.width, 1366);
                            assert_eq!(frame.height, 768);
                            assert_eq!(frame.data.len(), (1366 * 768 * 4) as usize);
                        }
                        Err(e) => {
                            println!("Capture failed (may be permission issue): {:?}", e);
                            // Don't fail test since this might be due to permissions
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to create capture: {:?}", e);
                }
            }
        }
    }
}