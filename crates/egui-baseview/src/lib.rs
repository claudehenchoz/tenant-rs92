//! Vendored into TENANT RS-92 from egui-baseview rev ec70c3fe (MIT, see LICENSE).
//! Deviation from upstream: the screen rect, pointer and scroll positions and the render
//! scale account for egui's zoom factor (`Context::set_zoom_factor`), which upstream ignored.
//! TENANT RS-92 zooms its fixed-size panel to fill a resizable window.

mod renderer;
mod translate;
mod window;

pub use window::{EguiWindow, Queue};

pub use egui;
pub use renderer::GraphicsConfig;
