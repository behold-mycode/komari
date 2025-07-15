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
}

pub fn query_capture_handles() -> Vec<(String, Handle)> {
    use screenshots::Screen;
    
    let mut handles = Vec::new();
    
    // Enumerate all available displays
    if let Ok(screens) = Screen::all() {
        for (index, screen) in screens.iter().enumerate() {
            let display_info = &screen.display_info;
            let name = format!(
                "Display {} ({}×{}) {}", 
                index, 
                display_info.width, 
                display_info.height,
                if display_info.is_primary { "[Primary]" } else { "" }
            );
            
            let handle = Handle::new_fixed(index as u64)
                .with_coordinates(index, 0, 0, 1366, 768);
            
            handles.push((name, handle));
        }
    }
    
    // If no displays found, provide fallback
    if handles.is_empty() {
        handles.push((
            "Default Display (Manual Config Required)".to_string(), 
            Handle::new_fixed(0)
        ));
    }
    
    handles
}