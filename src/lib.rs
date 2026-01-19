//! viewarr - A browser-based array/image viewer using Rust, WASM, and egui
//!
//! This library provides a WebAssembly-based image viewer that can be embedded
//! in web applications. It accepts typed array data from JavaScript and renders
//! it with colormap support, showing pixel values on hover.
//!
//! ## Architecture
//!
//! - `ArrayViewerWidget`: Self-contained egui widget with all viewing state
//! - `ViewerApp`: Thin eframe App shell that hosts the widget
//! - `ViewerHandle`: WASM interface for JavaScript to control the viewer

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod app;
mod colormap;
mod colormap_luts;
mod transform;
mod widget;

use app::ViewerApp;
use widget::ArrayViewerWidget;

/// A handle to a viewer instance. Each handle manages its own canvas and state.
///
/// This struct is exposed to JavaScript and provides methods to control the viewer.
/// It holds an Rc to the widget so it can call methods on it, and also stores
/// the eframe runner for the application lifecycle.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct ViewerHandle {
    /// The widget instance (shared with ViewerApp)
    widget: Rc<RefCell<ArrayViewerWidget>>,
    /// The eframe runner (kept alive to maintain the render loop)
    #[allow(dead_code)]
    runner: eframe::WebRunner,
}

#[wasm_bindgen(start)]
fn init_logging() {
    // Initialize the logger
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
}


#[wasm_bindgen]
impl ViewerHandle {
    /// Create a new viewer instance attached to the given canvas element.
    /// Returns a promise that resolves to a ViewerHandle when initialization completes.
    /// 
    /// Use this static factory method instead of a constructor since async constructors
    /// are deprecated in wasm-bindgen.
    #[wasm_bindgen]
    pub async fn create(canvas: HtmlCanvasElement) -> Result<ViewerHandle, JsValue> {
        // Initialize logging for debug builds
        #[cfg(debug_assertions)]
        {
            eframe::WebLogger::init(log::LevelFilter::Debug).ok();
        }
        #[cfg(not(debug_assertions))]
        {
            eframe::WebLogger::init(log::LevelFilter::Warn).ok();
        }

        // Create the widget that will be shared between the handle and the app
        let widget = Rc::new(RefCell::new(ArrayViewerWidget::new()));
        let widget_for_app = widget.clone();

        let web_options = eframe::WebOptions::default();
        let runner = eframe::WebRunner::new();

        runner
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(ViewerApp::new(cc, widget_for_app.clone())))),
            )
            .await?;

        Ok(ViewerHandle { widget, runner })
    }

    /// Set the image data to display.
    ///
    /// # Arguments
    /// * `buffer` - ArrayBuffer containing the raw pixel data
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `array_type` - Rust-style type specifier:
    ///   - "i8" (signed 8-bit integer)
    ///   - "u8" (unsigned 8-bit integer)
    ///   - "i16" (signed 16-bit integer)
    ///   - "u16" (unsigned 16-bit integer)
    ///   - "i32" (signed 32-bit integer)
    ///   - "u32" (unsigned 32-bit integer)
    ///   - "i64" (signed 64-bit integer)
    ///   - "u64" (unsigned 64-bit integer)
    ///   - "f32" (32-bit float)
    ///   - "f64" (64-bit float, default)
    #[wasm_bindgen(js_name = setImageData)]
    pub fn set_image_data(
        &self,
        buffer: &js_sys::ArrayBuffer,
        width: u32,
        height: u32,
        array_type: &str,
    ) -> Result<(), JsValue> {
        let pixels = convert_buffer_to_f64(buffer, array_type)?;

        let expected_len = (width as usize) * (height as usize);
        if pixels.len() != expected_len {
            return Err(JsValue::from_str(&format!(
                "Buffer size mismatch: expected {} pixels ({}x{}), got {}",
                expected_len,
                width,
                height,
                pixels.len()
            )));
        }

        // Determine if the source data is integer-typed (for display formatting)
        let is_integer = matches!(
            array_type,
            "i8" | "u8" | "i16" | "u16" |
            "i32" | "u32" | "i64" | "u64"
        );

        let mut widget = self.widget.borrow_mut();
        widget.set_image(pixels, width, height, is_integer);

        Ok(())
    }

    /// End event loop and release resources
    #[wasm_bindgen(js_name = destroy)]
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    /// Zoom in by one step (1.25x)
    #[wasm_bindgen(js_name = zoomIn)]
    pub fn zoom_in(&self) {
        let mut widget = self.widget.borrow_mut();
        // Use a default viewport center - the actual center will be calculated in the UI
        let center = egui::pos2(400.0, 300.0);
        widget.zoom_in(None, center);
    }

    /// Zoom out by one step (1/1.25x)
    #[wasm_bindgen(js_name = zoomOut)]
    pub fn zoom_out(&self) {
        let mut widget = self.widget.borrow_mut();
        let center = egui::pos2(400.0, 300.0);
        widget.zoom_out(None, center);
    }

    /// Reset zoom and pan to fit-to-view
    #[wasm_bindgen(js_name = zoomToFit)]
    pub fn zoom_to_fit(&self) {
        let mut widget = self.widget.borrow_mut();
        widget.zoom_to_fit();
    }

    /// Set zoom level directly (1.0 = fit to view)
    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&self, level: f32) {
        let mut widget = self.widget.borrow_mut();
        let transform = widget.transform_mut();
        transform.zoom = level.clamp(transform::MIN_ZOOM, transform::MAX_ZOOM);
    }

    /// Get current zoom level (1.0 = fit to view)
    #[wasm_bindgen(js_name = getZoom)]
    pub fn get_zoom(&self) -> f32 {
        self.widget.borrow().zoom_level()
    }
}

/// Convert a JavaScript ArrayBuffer to Vec<f64> based on ArrayType string.
/// ArrayType values are Rust-style type specifiers (i8, u8, i16, etc.).
#[cfg(target_arch = "wasm32")]
fn convert_buffer_to_f64(buffer: &js_sys::ArrayBuffer, array_type: &str) -> Result<Vec<f64>, JsValue> {
    match array_type {
        "i8" => {
            let view = js_sys::Int8Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u8" => {
            let view = js_sys::Uint8Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i16" => {
            let view = js_sys::Int16Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u16" => {
            let view = js_sys::Uint16Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i32" => {
            let view = js_sys::Int32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u32" => {
            let view = js_sys::Uint32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i64" => {
            let view = js_sys::BigInt64Array::new(buffer);
            let len = view.length() as usize;
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                let val = view.get_index(i as u32);
                // i64 converts to f64 directly (may lose precision for very large values)
                result.push(val as f64);
            }
            Ok(result)
        }
        "u64" => {
            let view = js_sys::BigUint64Array::new(buffer);
            let len = view.length() as usize;
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                let val = view.get_index(i as u32);
                // u64 converts to f64 directly (may lose precision for very large values)
                result.push(val as f64);
            }
            Ok(result)
        }
        "f32" => {
            let view = js_sys::Float32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "f64" | _ => {
            // Default to Float64
            let view = js_sys::Float64Array::new(buffer);
            Ok(view.to_vec())
        }
    }
}
