//! egui application state and rendering logic

use egui::{Color32, ColorImage, Key, PointerButton, TextureHandle, TextureOptions, Vec2};

use crate::colormap::Colormap;
use crate::transform::ViewTransform;

/// Default contrast value (DS9 default)
const DEFAULT_CONTRAST: f64 = 1.0;
/// Default bias value (DS9 default)
const DEFAULT_BIAS: f64 = 0.5;
/// Maximum contrast value (DS9 uses 0-10 range)
const MAX_CONTRAST: f64 = 10.0;
/// Minimum contrast value
const MIN_CONTRAST: f64 = 0.0;
/// Log stretch exponent (DS9 default for optical images)
const LOG_EXPONENT: f64 = 1000.0;
/// Color bar width in pixels
const COLORBAR_WIDTH: f32 = 32.0;
/// Maximum color bar height in pixels
const COLORBAR_MAX_HEIGHT: f32 = 300.0;
/// Color bar margin from edge
const COLORBAR_MARGIN: f32 = 10.0;

/// Stretch function type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StretchType {
    Linear,
    Log,
}

impl Default for StretchType {
    fn default() -> Self {
        Self::Linear
    }
}

/// Contrast and bias settings for a stretch mode
#[derive(Clone, Copy, Debug)]
pub struct ContrastBias {
    pub contrast: f64,
    pub bias: f64,
}

impl Default for ContrastBias {
    fn default() -> Self {
        Self {
            contrast: DEFAULT_CONTRAST,
            bias: DEFAULT_BIAS,
        }
    }
}

impl ContrastBias {
    pub fn is_default(&self) -> bool {
        (self.contrast - DEFAULT_CONTRAST).abs() < 0.001
            && (self.bias - DEFAULT_BIAS).abs() < 0.001
    }
}

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
    /// Pan/zoom transformation state
    transform: ViewTransform,
    /// Current stretch type (Linear or Log)
    stretch_type: StretchType,
    /// Contrast/bias settings for Linear mode
    linear_cb: ContrastBias,
    /// Contrast/bias settings for Log mode
    log_cb: ContrastBias,
    /// Whether user is currently dragging to adjust contrast/bias
    is_adjusting_stretch: bool,
    /// Current colormap
    colormap: Colormap,
    /// Symmetric mode (scale around zero)
    symmetric_mode: bool,
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
            transform: ViewTransform::new(),
            stretch_type: StretchType::default(),
            linear_cb: ContrastBias::default(),
            log_cb: ContrastBias::default(),
            is_adjusting_stretch: false,
            colormap: Colormap::default(),
            symmetric_mode: false,
        }
    }

    /// Set new image data, computing min/max for auto-scaling
    /// Pan is reset if dimensions change; zoom is always preserved
    pub fn set_image(&mut self, pixels: Vec<f64>, width: u32, height: u32, is_integer: bool) {
        // Check if dimensions changed
        let dimensions_changed = width != self.width || height != self.height;

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

        // Only reset pan if dimensions changed; always keep zoom
        if dimensions_changed {
            self.transform.reset_pan();
        }
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

    /// Build a ColorImage from the current pixel data using colormap
    /// with stretch function and contrast/bias applied
    pub fn build_color_image(&self) -> Option<ColorImage> {
        let pixels = self.pixels.as_ref()?;

        let (scale_min, scale_max) = self.scaling_range();
        let cb = self.current_contrast_bias();
        let stretch_type = self.stretch_type;
        let colormap = self.colormap;

        let rgba: Vec<Color32> = pixels
            .iter()
            .map(|&v| {
                let adjusted = self.apply_full_stretch(v, scale_min, scale_max, cb, stretch_type);
                colormap.map(adjusted)
            })
            .collect();

        Some(ColorImage {
            size: [self.width as usize, self.height as usize],
            pixels: rgba,
        })
    }

    /// Get the scaling range based on symmetric mode
    pub fn scaling_range(&self) -> (f64, f64) {
        if self.symmetric_mode {
            let abs_max = self.min_val.abs().max(self.max_val.abs());
            (-abs_max, abs_max)
        } else {
            (self.min_val, self.max_val)
        }
    }

    /// Apply full stretch pipeline to a single value
    /// Returns a value in 0-1 range suitable for colormap lookup
    fn apply_full_stretch(
        &self,
        v: f64,
        scale_min: f64,
        scale_max: f64,
        cb: ContrastBias,
        stretch_type: StretchType,
    ) -> f64 {
        // Step 1: Normalize to 0-1
        let range = scale_max - scale_min;
        let normalized = if v.is_finite() && range.abs() > f64::EPSILON {
            ((v - scale_min) / range).clamp(0.0, 1.0)
        } else {
            0.0 // NaN/Inf -> black
        };

        // Step 2: Apply stretch function
        let stretched = apply_stretch(normalized, stretch_type);

        // Step 3: Apply contrast/bias (DS9 formula)
        apply_contrast_bias(stretched, cb.contrast, cb.bias)
    }

    /// Apply stretch to a normalized value for colorbar generation
    pub fn apply_colorbar_stretch(&self, t: f64) -> f64 {
        let cb = self.current_contrast_bias();
        let stretched = apply_stretch(t, self.stretch_type);
        apply_contrast_bias(stretched, cb.contrast, cb.bias)
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

    /// Get mutable reference to transform
    pub fn transform_mut(&mut self) -> &mut ViewTransform {
        &mut self.transform
    }

    /// Get reference to transform
    pub fn transform(&self) -> &ViewTransform {
        &self.transform
    }

    /// Zoom in by one step
    pub fn zoom_in(&mut self, center: Option<egui::Pos2>, viewport_center: egui::Pos2) {
        self.transform.zoom_in(center, viewport_center);
    }

    /// Zoom out by one step
    pub fn zoom_out(&mut self, center: Option<egui::Pos2>, viewport_center: egui::Pos2) {
        self.transform.zoom_out(center, viewport_center);
    }

    /// Reset to fit-to-view
    pub fn zoom_to_fit(&mut self) {
        self.transform.reset();
    }

    /// Get current zoom level (1.0 = fit to view)
    pub fn zoom_level(&self) -> f32 {
        self.transform.zoom
    }

    /// Check if view is at default state
    pub fn is_default_view(&self) -> bool {
        self.transform.is_default()
    }

    /// Get current stretch type
    pub fn stretch_type(&self) -> StretchType {
        self.stretch_type
    }

    /// Toggle between Linear and Log stretch
    pub fn toggle_stretch_type(&mut self) {
        self.stretch_type = match self.stretch_type {
            StretchType::Linear => StretchType::Log,
            StretchType::Log => StretchType::Linear,
        };
        self.texture_dirty = true;
    }

    /// Get current contrast/bias for the active stretch mode
    pub fn current_contrast_bias(&self) -> ContrastBias {
        match self.stretch_type {
            StretchType::Linear => self.linear_cb,
            StretchType::Log => self.log_cb,
        }
    }

    /// Get mutable reference to current contrast/bias
    fn current_contrast_bias_mut(&mut self) -> &mut ContrastBias {
        match self.stretch_type {
            StretchType::Linear => &mut self.linear_cb,
            StretchType::Log => &mut self.log_cb,
        }
    }

    /// Adjust contrast/bias based on mouse drag delta
    /// dx: horizontal delta (mapped to bias)
    /// dy: vertical delta (mapped to contrast)
    /// viewport_size: for normalizing the delta
    pub fn adjust_contrast_bias(&mut self, dx: f32, dy: f32, viewport_size: Vec2) {
        let cb = self.current_contrast_bias_mut();

        // Map horizontal to bias (0 to 1)
        cb.bias = (cb.bias + (dx as f64) / (viewport_size.x as f64)).clamp(0.0, 1.0);

        // Map vertical to contrast (0 to MAX_CONTRAST)
        // Negate dy because screen Y increases downward but we want drag-up to increase contrast
        cb.contrast = (cb.contrast - (dy as f64) / (viewport_size.y as f64) * MAX_CONTRAST)
            .clamp(MIN_CONTRAST, MAX_CONTRAST);

        self.texture_dirty = true;
    }

    /// Reset contrast/bias for current stretch mode to defaults
    pub fn reset_current_stretch(&mut self) {
        *self.current_contrast_bias_mut() = ContrastBias::default();
        self.texture_dirty = true;
    }

    /// Reset all stretch settings (both modes) to defaults
    pub fn reset_all_stretch(&mut self) {
        self.linear_cb = ContrastBias::default();
        self.log_cb = ContrastBias::default();
        self.stretch_type = StretchType::Linear;
        self.texture_dirty = true;
    }

    /// Check if current stretch mode has non-default contrast/bias
    pub fn is_stretch_modified(&self) -> bool {
        !self.current_contrast_bias().is_default()
    }

    /// Set whether user is currently adjusting stretch
    pub fn set_adjusting_stretch(&mut self, adjusting: bool) {
        self.is_adjusting_stretch = adjusting;
    }

    /// Check if user is currently adjusting stretch
    pub fn is_adjusting_stretch(&self) -> bool {
        self.is_adjusting_stretch
    }

    /// Get current colormap
    pub fn colormap(&self) -> Colormap {
        self.colormap
    }

    /// Set colormap
    pub fn set_colormap(&mut self, colormap: Colormap) {
        // If selecting a diverging colormap, enable symmetric mode
        if colormap.is_diverging() && !self.symmetric_mode {
            self.symmetric_mode = true;
        }
        self.colormap = colormap;
        self.texture_dirty = true;
    }

    /// Check if symmetric mode is enabled
    pub fn is_symmetric(&self) -> bool {
        self.symmetric_mode
    }

    /// Toggle symmetric mode
    pub fn toggle_symmetric(&mut self) {
        self.symmetric_mode = !self.symmetric_mode;
        // If disabling symmetric mode and using a diverging colormap, switch to grayscale
        if !self.symmetric_mode && self.colormap.is_diverging() {
            self.colormap = Colormap::Grayscale;
        }
        self.texture_dirty = true;
    }

    /// Set stretch type directly (used by selectable labels)
    pub fn set_stretch_type(&mut self, stretch_type: StretchType) {
        if self.stretch_type != stretch_type {
            self.stretch_type = stretch_type;
            // If switching to log, disable symmetric mode (log doesn't work well with negative values)
            if stretch_type == StretchType::Log && self.symmetric_mode {
                self.symmetric_mode = false;
                // Also switch away from diverging colormaps
                if self.colormap.is_diverging() {
                    self.colormap = Colormap::Grayscale;
                }
            }
            self.texture_dirty = true;
        }
    }
}

/// Apply stretch function to a normalized value (0-1)
fn apply_stretch(x: f64, stretch_type: StretchType) -> f64 {
    match stretch_type {
        StretchType::Linear => x,
        StretchType::Log => {
            // DS9 log formula: log10(a*x + 1) / log10(a)
            // This naturally handles x=0: log10(1) = 0
            (LOG_EXPONENT * x + 1.0).log10() / LOG_EXPONENT.log10()
        }
    }
}

/// Apply DS9-style contrast/bias transformation
/// Formula: ((x - bias) * contrast + 0.5).clamp(0, 1)
fn apply_contrast_bias(x: f64, contrast: f64, bias: f64) -> f64 {
    ((x - bias) * contrast + 0.5).clamp(0.0, 1.0)
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
    /// Colorbar texture (regenerated when stretch changes)
    colorbar_texture: Option<TextureHandle>,
    /// Track if right mouse button started a drag (for contrast/bias adjustment)
    stretch_drag_active: bool,
}

impl ViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: std::rc::Rc<std::cell::RefCell<AppState>>,
    ) -> Self {
        Self {
            state,
            texture: None,
            colorbar_texture: None,
            stretch_drag_active: false,
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
        // Also rebuild colorbar
        let colormap = state.colormap();
        let cb = state.current_contrast_bias();
        let stretch_type = state.stretch_type();
        drop(state);

        self.rebuild_colorbar_texture(ctx, colormap, cb, stretch_type);
    }

    /// Rebuild the colorbar texture
    fn rebuild_colorbar_texture(
        &mut self,
        ctx: &egui::Context,
        colormap: Colormap,
        cb: ContrastBias,
        stretch_type: StretchType,
    ) {
        let height = 256;
        let width = 1;

        let pixels: Vec<Color32> = (0..height)
            .rev() // Reverse so high values are at top
            .map(|y| {
                let t = y as f64 / (height - 1) as f64;
                let stretched = apply_stretch(t, stretch_type);
                let adjusted = apply_contrast_bias(stretched, cb.contrast, cb.bias);
                colormap.map(adjusted)
            })
            .collect();

        let color_image = ColorImage {
            size: [width, height],
            pixels,
        };

        self.colorbar_texture = Some(ctx.load_texture(
            "colorbar",
            color_image,
            TextureOptions::LINEAR,
        ));
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

        // Handle keyboard shortcuts (before panel to capture globally)
        self.handle_keyboard_input(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let state_ref = self.state.borrow();

            if !state_ref.has_image() {
                ui.centered_and_justified(|ui| {
                    ui.label("No image loaded");
                });
                return;
            }

            let (img_width, img_height) = state_ref.dimensions();
            let transform = state_ref.transform().clone();
            drop(state_ref); // Release borrow for mutable access later

            // Calculate base image display size (fit-to-view size)
            let available_size = ui.available_size();
            let img_aspect = img_width as f32 / img_height as f32;
            let available_aspect = available_size.x / available_size.y;

            let base_display_size = if img_aspect > available_aspect {
                // Image is wider than available space
                egui::vec2(available_size.x, available_size.x / img_aspect)
            } else {
                // Image is taller than available space
                egui::vec2(available_size.y * img_aspect, available_size.y)
            };

            // Calculate the actual image rect with zoom and pan applied
            let viewport_rect = ui.max_rect();
            let image_rect = transform.calculate_image_rect(viewport_rect, base_display_size);
            let viewport_center = viewport_rect.center();

            // Allocate the full available space for interaction
            let (full_rect, response) = ui.allocate_exact_size(
                available_size,
                egui::Sense::click_and_drag(),
            );

            // Draw the image at the transformed position
            if let Some(texture) = &self.texture {
                // Use a painter to draw at arbitrary position
                let painter = ui.painter_at(full_rect);
                // Flip Y-axis for FITS convention: Y=0 at bottom
                // UV coords go from (0,1) at top-left to (1,0) at bottom-right
                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
                    egui::Color32::WHITE,
                );
            }

            // Handle mouse wheel zoom
            let zoom_delta = ui.input(|i| i.zoom_delta());
            if zoom_delta != 1.0 {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
                    if response.rect.contains(pointer_pos) {
                        let mut state = self.state.borrow_mut();
                        state.transform_mut().zoom_around_point(zoom_delta, pointer_pos, viewport_center);
                    }
                }
            }

            // Handle scroll wheel (for zoom when not using native zoom gesture)
            let scroll_delta = ui.input(|i| i.raw_scroll_delta);
            if scroll_delta.y != 0.0 && zoom_delta == 1.0 {
                // Scroll without pinch gesture - use for zoom centered on cursor
                if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
                    if response.rect.contains(pointer_pos) {
                        // Use smaller step for scroll wheel for finer control
                        let zoom_factor = if scroll_delta.y > 0.0 {
                            crate::transform::SCROLL_ZOOM_STEP
                        } else {
                            1.0 / crate::transform::SCROLL_ZOOM_STEP
                        };
                        let mut state = self.state.borrow_mut();
                        state.transform_mut().zoom_around_point(zoom_factor, pointer_pos, viewport_center);
                    }
                }
            }

            // Handle pan via drag
            // Primary click-drag pans by default, middle mouse also pans
            let should_pan = response.dragged_by(PointerButton::Primary)
                || response.dragged_by(PointerButton::Middle);

            if should_pan {
                let drag_delta = response.drag_delta();
                if drag_delta != Vec2::ZERO {
                    let mut state = self.state.borrow_mut();
                    state.transform_mut().pan_by(drag_delta);
                }
            }

            // Handle contrast/bias adjustment via right-click drag (DS9 style)
            if response.drag_started_by(PointerButton::Secondary) {
                self.stretch_drag_active = true;
                let mut state = self.state.borrow_mut();
                state.set_adjusting_stretch(true);
            }

            if self.stretch_drag_active && response.dragged_by(PointerButton::Secondary) {
                let drag_delta = response.drag_delta();
                if drag_delta != Vec2::ZERO {
                    let mut state = self.state.borrow_mut();
                    state.adjust_contrast_bias(drag_delta.x, drag_delta.y, available_size);
                }
            }

            if response.drag_stopped_by(PointerButton::Secondary) {
                self.stretch_drag_active = false;
                let mut state = self.state.borrow_mut();
                state.set_adjusting_stretch(false);
            }

            // Handle alt+click to center on point
            let modifiers = ui.input(|i| i.modifiers);
            if response.clicked() && modifiers.alt {
                if let Some(click_pos) = response.interact_pointer_pos() {
                    let state_ref = self.state.borrow();
                    let transform = state_ref.transform();
                    if let Some((img_x, img_y)) = transform.screen_to_image(
                        click_pos,
                        image_rect,
                        (img_width, img_height),
                    ) {
                        drop(state_ref);
                        let mut state = self.state.borrow_mut();
                        state.transform_mut().center_on_image_point(
                            egui::pos2(img_x as f32, img_y as f32),
                            egui::vec2(img_width as f32, img_height as f32),
                            available_size,
                            egui::Rect::from_center_size(viewport_center, base_display_size),
                        );
                    }
                }
            }

            // Handle hover to show pixel value
            if let Some(hover_pos) = response.hover_pos() {
                let state_ref = self.state.borrow();
                let transform = state_ref.transform();
                if let Some((img_x, img_y)) = transform.screen_to_image(
                    hover_pos,
                    image_rect,
                    (img_width, img_height),
                ) {
                    drop(state_ref);
                    let mut state = self.state.borrow_mut();
                    if let Some(value) = state.get_pixel_value(img_x, img_y) {
                        state.set_hover_info(Some((img_x, img_y, value)));
                    } else {
                        state.set_hover_info(None);
                    }
                } else {
                    drop(state_ref);
                    let mut state = self.state.borrow_mut();
                    state.set_hover_info(None);
                }
            } else {
                let mut state = self.state.borrow_mut();
                state.set_hover_info(None);
            }

            // Render zoom controls overlay
            self.render_zoom_controls(ctx, viewport_center);

            // Render stretch controls overlay
            self.render_stretch_controls(ctx);

            // Render colorbar overlay
            self.render_colorbar(ctx);

            // Render contrast/bias info while adjusting
            self.render_stretch_info_overlay(ctx);

            // Display hover info overlay at bottom
            self.render_hover_overlay(ctx, ui);
        });

        // Request continuous repaints for smooth updates
        ctx.request_repaint();
    }
}

impl ViewerApp {
    /// Handle keyboard shortcuts for zoom
    fn handle_keyboard_input(&self, ctx: &egui::Context) {
        let viewport_center = ctx.screen_rect().center();

        ctx.input(|i| {
            // Zoom in: = or + (numpad)
            if i.key_pressed(Key::Equals) || i.key_pressed(Key::Plus) {
                let mut state = self.state.borrow_mut();
                state.zoom_in(None, viewport_center);
            }
            // Zoom out: - (minus)
            if i.key_pressed(Key::Minus) {
                let mut state = self.state.borrow_mut();
                state.zoom_out(None, viewport_center);
            }
            // Reset: 0
            if i.key_pressed(Key::Num0) {
                let mut state = self.state.borrow_mut();
                state.zoom_to_fit();
            }
        });
    }

    /// Render zoom control buttons at bottom-right
    fn render_zoom_controls(&self, ctx: &egui::Context, viewport_center: egui::Pos2) {
        let screen_rect = ctx.screen_rect();
        let button_size = egui::vec2(28.0, 28.0);
        let margin = 10.0;
        let spacing = 4.0;

        // Position at bottom-right (always reserve space for 3 buttons)
        let num_buttons = 3.0;
        let base_x = screen_rect.max.x - margin - button_size.x * num_buttons - spacing * (num_buttons - 1.0);
        let base_y = screen_rect.max.y - margin - button_size.y;

        egui::Area::new(egui::Id::new("zoom_controls"))
            .fixed_pos(egui::pos2(base_x, base_y))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;

                    // Reset button (always in same position, but only visible when not default)
                    let show_reset = !self.state.borrow().is_default_view();
                    if show_reset {
                        if ui.add_sized(button_size, egui::Button::new("⟲")).clicked() {
                            let mut state = self.state.borrow_mut();
                            state.zoom_to_fit();
                        }
                    } else {
                        // Invisible placeholder to maintain layout
                        ui.add_sized(button_size, egui::Label::new(""));
                    }

                    // Zoom out button
                    if ui.add_sized(button_size, egui::Button::new("−")).clicked() {
                        let mut state = self.state.borrow_mut();
                        state.zoom_out(None, viewport_center);
                    }

                    // Zoom in button
                    if ui.add_sized(button_size, egui::Button::new("+")).clicked() {
                        let mut state = self.state.borrow_mut();
                        state.zoom_in(None, viewport_center);
                    }
                });

                // Show zoom percentage
                let zoom_level = self.state.borrow().zoom_level();
                ui.label(format!("{:.0}%", zoom_level * 100.0));

                // Debug: show build timestamp
                ui.separator();
                ui.label(format!("Build: {}", env!("BUILD_TIMESTAMP")));
            });
    }

    /// Render stretch controls (log/linear toggle, colormap, symmetric, reset) at top-right
    fn render_stretch_controls(&self, ctx: &egui::Context) {
        let screen_rect = ctx.screen_rect();
        let margin = 10.0;

        egui::Area::new(egui::Id::new("stretch_controls"))
            .fixed_pos(egui::pos2(screen_rect.max.x - margin - 200.0, margin))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_black_alpha(180))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let state_ref = self.state.borrow();
                            let stretch_type = state_ref.stretch_type();
                            let is_modified = state_ref.is_stretch_modified();
                            let colormap = state_ref.colormap();
                            let symmetric = state_ref.is_symmetric();
                            drop(state_ref);

                            // Stretch type toggle using SelectableLabel
                            if ui.selectable_label(stretch_type == StretchType::Linear, "Lin").clicked() {
                                let mut state = self.state.borrow_mut();
                                state.set_stretch_type(StretchType::Linear);
                            }
                            if ui.selectable_label(stretch_type == StretchType::Log, "Log").clicked() {
                                let mut state = self.state.borrow_mut();
                                state.set_stretch_type(StretchType::Log);
                            }

                            ui.separator();

                            // Colormap selection
                            for &cmap in Colormap::standard_colormaps() {
                                if ui.selectable_label(colormap == cmap, cmap.name()).clicked() {
                                    let mut state = self.state.borrow_mut();
                                    state.set_colormap(cmap);
                                }
                            }

                            // Only show symmetric mode toggle for linear stretch
                            if stretch_type == StretchType::Linear {
                                ui.separator();

                                // Symmetric mode toggle
                                if ui.selectable_label(symmetric, "±").on_hover_text("Symmetric scaling").clicked() {
                                    let mut state = self.state.borrow_mut();
                                    state.toggle_symmetric();
                                }

                                // Diverging colormaps only available in symmetric mode
                                if symmetric {
                                    for &cmap in Colormap::diverging_colormaps() {
                                        if ui.selectable_label(colormap == cmap, cmap.name()).clicked() {
                                            let mut state = self.state.borrow_mut();
                                            state.set_colormap(cmap);
                                        }
                                    }
                                }
                            }

                            // Reset stretch button (only show when modified)
                            if is_modified {
                                ui.separator();
                                if ui.button("⟲").on_hover_text("Reset contrast/bias").clicked() {
                                    let mut state = self.state.borrow_mut();
                                    state.reset_current_stretch();
                                }
                            }
                        });
                    });
            });
    }

    /// Render colorbar overlay at top-left
    fn render_colorbar(&self, ctx: &egui::Context) {
        let state_ref = self.state.borrow();
        if !state_ref.has_image() {
            return;
        }

        let screen_rect = ctx.screen_rect();
        let (scale_min, scale_max) = state_ref.scaling_range();
        let is_int = state_ref.is_integer();
        drop(state_ref);

        // Calculate colorbar height: min(300, 0.5 * viewport_height)
        let bar_height = COLORBAR_MAX_HEIGHT.min(screen_rect.height() * 0.5);
        let bar_width = COLORBAR_WIDTH;

        egui::Area::new(egui::Id::new("colorbar"))
            .fixed_pos(egui::pos2(COLORBAR_MARGIN, COLORBAR_MARGIN))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Max value label at top
                    let max_label = if is_int {
                        format!("{}", scale_max as i64)
                    } else {
                        format_scientific(scale_max)
                    };
                    ui.label(egui::RichText::new(&max_label).color(Color32::WHITE).small());

                    // Draw colorbar
                    if let Some(texture) = &self.colorbar_texture {
                        let bar_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(bar_width, bar_height),
                        );
                        ui.allocate_space(egui::vec2(bar_width, bar_height));

                        let painter = ui.painter();
                        // Draw border
                        painter.rect_stroke(
                            bar_rect.expand(1.0),
                            0.0,
                            egui::Stroke::new(1.0, Color32::GRAY),
                        );
                        // Draw colorbar texture
                        painter.image(
                            texture.id(),
                            bar_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    // Min value label at bottom
                    let min_label = if is_int {
                        format!("{}", scale_min as i64)
                    } else {
                        format_scientific(scale_min)
                    };
                    ui.label(egui::RichText::new(&min_label).color(Color32::WHITE).small());
                });
            });
    }

    /// Render contrast/bias values while adjusting (DS9-style feedback)
    fn render_stretch_info_overlay(&self, ctx: &egui::Context) {
        let state_ref = self.state.borrow();
        if !state_ref.is_adjusting_stretch() {
            return;
        }

        let cb = state_ref.current_contrast_bias();
        let stretch_type = state_ref.stretch_type();
        drop(state_ref);

        let screen_rect = ctx.screen_rect();

        egui::Area::new(egui::Id::new("stretch_info_overlay"))
            .fixed_pos(egui::pos2(screen_rect.center().x - 80.0, 50.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_black_alpha(200))
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        let mode_str = match stretch_type {
                            StretchType::Linear => "Linear",
                            StretchType::Log => "Log",
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "{} | Contrast: {:.2} | Bias: {:.2}",
                                mode_str, cb.contrast, cb.bias
                            ))
                            .color(egui::Color32::WHITE),
                        );
                    });
            });
    }

    /// Render hover info overlay at bottom-left
    fn render_hover_overlay(&self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let state_ref = self.state.borrow();
        if let Some((x, y, value)) = state_ref.hover_info() {
            let is_int = state_ref.is_integer();

            // Create overlay at the bottom of the panel
            egui::Area::new(egui::Id::new("hover_overlay"))
                .fixed_pos(egui::pos2(10.0, ui.ctx().screen_rect().max.y - 30.0))
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        if is_int {
                            ui.label(format!("Pixel ({}, {}): {}", x, y, value as i64));
                        } else {
                            ui.label(format!("Pixel ({}, {}): {:.6}", x, y, value));
                        }
                    });
                });
        }
    }
}

/// Format a float in scientific notation for compact display
fn format_scientific(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else if v.abs() >= 1e4 || v.abs() < 1e-2 {
        format!("{:.2e}", v)
    } else {
        format!("{:.2}", v)
    }
}
