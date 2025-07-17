use super::{Error, Frame, Handle, screenshot::ScreenshotCapture};

#[derive(Debug)]
pub struct BitBltCapture {
    inner: ScreenshotCapture,
}

impl BitBltCapture {
    pub fn new(handle: Handle) -> Result<Self, Error> {
        let inner = ScreenshotCapture::new(handle)?;
        Ok(Self { inner })
    }

    pub fn new_with_coordinates(display_index: usize, x: i32, y: i32, width: i32, height: i32) -> Result<Self, Error> {
        let handle = Handle::new("MapleStory")
            .with_coordinates(display_index, x, y, width, height);
        Self::new(handle)
    }

    pub fn grab(&mut self) -> Result<Frame, Error> {
        self.inner.grab()
    }

    pub fn stop_capture(&mut self) {
        self.inner.stop_capture();
    }

    pub fn set_capture_region(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<(), Error> {
        self.inner.set_capture_region(x, y, width, height)
    }
}