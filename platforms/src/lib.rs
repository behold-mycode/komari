#![feature(str_from_raw_parts)]

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;
