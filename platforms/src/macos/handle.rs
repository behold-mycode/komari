use std::cell::Cell;

#[derive(Clone, Debug)]
pub(crate) struct HandleCell {
    handle: Handle,
    inner: Cell<Option<u64>>,
}

impl HandleCell {
    pub fn new(handle: Handle) -> Self {
        Self {
            handle,
            inner: Cell::new(None),
        }
    }

    #[inline]
    pub fn get_handle(&self) -> Handle {
        self.handle
    }

    #[inline]
    pub fn as_inner(&self) -> Option<u64> {
        match self.handle.kind {
            HandleKind::Fixed(id) => Some(id),
            HandleKind::Dynamic(_class) => {
                if self.inner.get().is_none() {
                    self.inner.set(self.handle.query_handle());
                }
                self.inner.get()
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HandleKind {
    Fixed(u64),
    Dynamic(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Handle {
    kind: HandleKind,
    pub display_index: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Handle {
    pub fn new(class: &'static str) -> Self {
        Self {
            kind: HandleKind::Dynamic(class),
            display_index: 0,
            x: 0,
            y: 0,
            width: 1366,
            height: 768,
        }
    }

    pub(crate) fn new_fixed(id: u64) -> Self {
        Self {
            kind: HandleKind::Fixed(id),
            display_index: 0,
            x: 0,
            y: 0,
            width: 1366,
            height: 768,
        }
    }

    pub fn with_coordinates(mut self, display_index: usize, x: i32, y: i32, width: i32, height: i32) -> Self {
        self.display_index = display_index;
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
        self
    }

    pub(crate) fn query_handle(&self) -> Option<u64> {
        match self.kind {
            HandleKind::Fixed(id) => Some(id),
            HandleKind::Dynamic(_class) => {
                // For macOS, we'll use a placeholder ID since we're doing coordinate-based capture
                // This maintains API compatibility while using BitBlt Area approach
                Some(1)
            }
        }
    }

    pub fn client_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        // Convert relative coordinates to absolute screen coordinates
        (self.x + x, self.y + y)
    }
    
    pub fn screen_to_client(&self, x: i32, y: i32) -> (i32, i32) {
        // Convert absolute screen coordinates to relative coordinates  
        (x - self.x, y - self.y)
    }

}

pub fn query_capture_handles() -> Vec<(String, Handle)> {
    let mut handles = Vec::new();
    
    // Provide capture options that match the UI expectations
    // The UI will allow users to configure coordinates through the settings
    handles.push((
        "MapleStory".to_string(),
        Handle::new("MapleStory")
    ));
    
    handles.push((
        "Screen Capture Area".to_string(),
        Handle::new_fixed(1)
    ));
    
    handles
}