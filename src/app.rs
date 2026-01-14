//! egui application state and rendering logic

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};

/// Shared state for a viewer instance
pub struct AppState {
    /// Raw pixel data as f64 values
    pixels: Option<Vec<f64>>,
    /// Image dimensions
    width: u32,
    height: u32,
    /// Computed min/max for scaling
    min_val: f64,
    max_val: f64,
    /// Container size (set from JS via ResizeObserver)
    container_width: u32,
    container_height: u32,
    /// Flag indicating image data has changed and texture needs rebuild
    texture_dirty: bool,
    /// Cached hover information: (image_x, image_y, raw_value)
    hover_info: Option<(u32, u32, f64)>,
    /// Whether the source data is integer-typed (for display formatting)
    is_integer: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            pixels: None,
            width: 0,
            height: 0,
            min_val: 0.0,
            max_val: 1.0,
            container_width: 800,
            container_height: 600,
            texture_dirty: false,
            hover_info: None,
            is_integer: false,
        }
    }

    /// Set new image data, computing min/max for auto-scaling
    pub fn set_image(&mut self, pixels: Vec<f64>, width: u32, height: u32, is_integer: bool) {
        // Compute min/max, ignoring NaN values
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;

        for &v in &pixels {
            if v.is_finite() {
                if v < min_val {
                    min_val = v;
                }
                if v > max_val {
                    max_val = v;
                }
            }
        }

        // Handle edge cases
        if !min_val.is_finite() {
            min_val = 0.0;
        }
        if !max_val.is_finite() {
            max_val = 1.0;
        }
        if (max_val - min_val).abs() < f64::EPSILON {
            max_val = min_val + 1.0;
        }

        self.pixels = Some(pixels);
        self.width = width;
        self.height = height;
        self.min_val = min_val;
        self.max_val = max_val;
        self.texture_dirty = true;
        self.is_integer = is_integer;
    }

    /// Update container size
    pub fn set_container_size(&mut self, width: u32, height: u32) {
        self.container_width = width;
        self.container_height = height;
    }

    /// Check if we have image data
    pub fn has_image(&self) -> bool {
        self.pixels.is_some()
    }

    /// Get image dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check and clear the dirty flag
    pub fn take_texture_dirty(&mut self) -> bool {
        let dirty = self.texture_dirty;
        self.texture_dirty = false;
        dirty
    }

    /// Build a ColorImage from the current pixel data using grayscale mapping
    pub fn build_color_image(&self) -> Option<ColorImage> {
        let pixels = self.pixels.as_ref()?;

        let range = self.max_val - self.min_val;
        let rgba: Vec<Color32> = pixels
            .iter()
            .map(|&v| {
                let normalized = if v.is_finite() {
                    ((v - self.min_val) / range).clamp(0.0, 1.0)
                } else {
                    0.0 // NaN/Inf -> black
                };
                let gray = (normalized * 255.0) as u8;
                Color32::from_gray(gray)
            })
            .collect();

        Some(ColorImage {
            size: [self.width as usize, self.height as usize],
            pixels: rgba,
        })
    }

    /// Get raw pixel value at image coordinates
    pub fn get_pixel_value(&self, x: u32, y: u32) -> Option<f64> {
        let pixels = self.pixels.as_ref()?;
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            pixels.get(idx).copied()
        } else {
            None
        }
    }

    /// Update hover info
    pub fn set_hover_info(&mut self, info: Option<(u32, u32, f64)>) {
        self.hover_info = info;
    }

    /// Get current hover info
    pub fn hover_info(&self) -> Option<(u32, u32, f64)> {
        self.hover_info
    }

    /// Get min/max values
    pub fn value_range(&self) -> (f64, f64) {
        (self.min_val, self.max_val)
    }

    /// Check if source data is integer-typed
    pub fn is_integer(&self) -> bool {
        self.is_integer
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// The egui application for the viewer
pub struct ViewerApp {
    state: std::rc::Rc<std::cell::RefCell<AppState>>,
    texture: Option<TextureHandle>,
}

impl ViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: std::rc::Rc<std::cell::RefCell<AppState>>,
    ) -> Self {
        Self {
            state,
            texture: None,
        }
    }

    /// Rebuild the texture from current image data
    fn rebuild_texture(&mut self, ctx: &egui::Context) {
        let state = self.state.borrow();
        if let Some(color_image) = state.build_color_image() {
            self.texture = Some(ctx.load_texture(
                "image",
                color_image,
                TextureOptions::NEAREST, // Use nearest-neighbor for pixel art look
            ));
        }
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if texture needs rebuilding
        {
            let mut state = self.state.borrow_mut();
            if state.take_texture_dirty() {
                drop(state); // Release borrow before rebuilding
                self.rebuild_texture(ctx);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let state_ref = self.state.borrow();

            if !state_ref.has_image() {
                ui.centered_and_justified(|ui| {
                    ui.label("No image loaded");
                });
                return;
            }

            let (img_width, img_height) = state_ref.dimensions();
            drop(state_ref); // Release borrow for mutable access later

            // Calculate image display size to fit in available space while maintaining aspect ratio
            let available_size = ui.available_size();
            let img_aspect = img_width as f32 / img_height as f32;
            let available_aspect = available_size.x / available_size.y;

            let display_size = if img_aspect > available_aspect {
                // Image is wider than available space
                egui::vec2(available_size.x, available_size.x / img_aspect)
            } else {
                // Image is taller than available space
                egui::vec2(available_size.y * img_aspect, available_size.y)
            };

            // Center the image
            let offset = (available_size - display_size) / 2.0;
            ui.add_space(offset.y);

            ui.horizontal(|ui| {
                ui.add_space(offset.x);

                if let Some(texture) = &self.texture {
                    let response = ui.add(
                        egui::Image::new(texture)
                            .fit_to_exact_size(display_size)
                            .sense(egui::Sense::hover()),
                    );

                    // Handle hover to show pixel value
                    if let Some(hover_pos) = response.hover_pos() {
                        let image_rect = response.rect;

                        // Convert screen position to image coordinates
                        let rel_x = (hover_pos.x - image_rect.min.x) / image_rect.width();
                        let rel_y = (hover_pos.y - image_rect.min.y) / image_rect.height();

                        let img_x = (rel_x * img_width as f32) as u32;
                        let img_y = (rel_y * img_height as f32) as u32;

                        let mut state = self.state.borrow_mut();
                        if let Some(value) = state.get_pixel_value(img_x, img_y) {
                            state.set_hover_info(Some((img_x, img_y, value)));
                        } else {
                            state.set_hover_info(None);
                        }
                    } else {
                        let mut state = self.state.borrow_mut();
                        state.set_hover_info(None);
                    }
                }
            });

            // Display hover info overlay at bottom
            let state_ref = self.state.borrow();
            if let Some((x, y, value)) = state_ref.hover_info() {
                let (min_val, max_val) = state_ref.value_range();
                let is_int = state_ref.is_integer();

                // Create overlay at the bottom of the panel
                egui::Area::new(egui::Id::new("hover_overlay"))
                    .fixed_pos(egui::pos2(10.0, ui.ctx().screen_rect().max.y - 30.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            if is_int {
                                ui.label(format!(
                                    "Pixel ({}, {}): {}  |  Range: [{}, {}]",
                                    x, y, value as i64, min_val as i64, max_val as i64
                                ));
                            } else {
                                ui.label(format!(
                                    "Pixel ({}, {}): {:.6}  |  Range: [{:.6}, {:.6}]",
                                    x, y, value, min_val, max_val
                                ));
                            }
                        });
                    });
            }
        });

        // Request continuous repaints while hovering for smooth updates
        ctx.request_repaint();
    }
}
