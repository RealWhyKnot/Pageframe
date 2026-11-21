mod baseline;
#[cfg(target_os = "windows")]
mod coverage;
#[cfg(target_os = "windows")]
mod glyphs;
#[cfg(target_os = "windows")]
mod memory;
#[cfg(target_os = "windows")]
mod raster;

pub use baseline::hinted_baseline;
#[cfg(target_os = "windows")]
pub use glyphs::{Axes, FontSource, GlyphPaint, GlyphRun, supported};
