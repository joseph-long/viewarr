//! viewarr - A browser-based array/image viewer using Rust, WASM, and egui
//!
//! This library provides a WebAssembly-based image viewer that can be embedded
//! in web applications. It accepts typed array data from JavaScript and renders
//! it with grayscale colormapping, showing pixel values on hover.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod app;

use app::AppState;

/// A handle to a viewer instance. Each handle manages its own canvas and state.
#[wasm_bindgen]
pub struct ViewerHandle {
    state: Rc<RefCell<AppState>>,
    #[allow(dead_code)]
    runner: eframe::WebRunner,
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

        let state = Rc::new(RefCell::new(AppState::new()));
        let state_clone = state.clone();

        let web_options = eframe::WebOptions::default();
        let runner = eframe::WebRunner::new();

        runner
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::ViewerApp::new(cc, state_clone.clone())))),
            )
            .await?;

        Ok(ViewerHandle { state, runner })
    }

    /// Set the image data to display.
    ///
    /// # Arguments
    /// * `buffer` - ArrayBuffer containing the raw pixel data
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `dtype` - Data type string matching numpy dtype conventions:
    ///   - "i1", "b" -> Int8
    ///   - "u1", "B" -> Uint8
    ///   - "i2" -> Int16
    ///   - "u2" -> Uint16
    ///   - "i4" -> Int32
    ///   - "u4" -> Uint32
    ///   - "i8" -> BigInt64
    ///   - "u8" -> BigUint64
    ///   - "f4" -> Float32
    ///   - "f8" -> Float64 (default)
    #[wasm_bindgen(js_name = setImageData)]
    pub fn set_image_data(
        &self,
        buffer: &js_sys::ArrayBuffer,
        width: u32,
        height: u32,
        dtype: &str,
    ) -> Result<(), JsValue> {
        let pixels = convert_buffer_to_f64(buffer, dtype)?;

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

        let mut state = self.state.borrow_mut();
        state.set_image(pixels, width, height);

        Ok(())
    }

    /// Notify the viewer that the container size has changed.
    /// The viewer will adjust its rendering on the next frame.
    #[wasm_bindgen(js_name = notifyResize)]
    pub fn notify_resize(&self, width: u32, height: u32) {
        let mut state = self.state.borrow_mut();
        state.set_container_size(width, height);
    }
}

/// Convert a JavaScript ArrayBuffer to Vec<f64> based on dtype string
fn convert_buffer_to_f64(buffer: &js_sys::ArrayBuffer, dtype: &str) -> Result<Vec<f64>, JsValue> {
    match dtype {
        "i1" | "b" => {
            let view = js_sys::Int8Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u1" | "B" => {
            let view = js_sys::Uint8Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i2" => {
            let view = js_sys::Int16Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u2" => {
            let view = js_sys::Uint16Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i4" => {
            let view = js_sys::Int32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "u4" => {
            let view = js_sys::Uint32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "i8" => {
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
        "u8" => {
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
        "f4" => {
            let view = js_sys::Float32Array::new(buffer);
            Ok(view.to_vec().into_iter().map(|v| v as f64).collect())
        }
        "f8" | _ => {
            // Default to Float64
            let view = js_sys::Float64Array::new(buffer);
            Ok(view.to_vec())
        }
    }
}
