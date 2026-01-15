//! ArrayViewerWidget - A self-contained egui widget for viewing 2D arrays/images
//!
//! This widget encapsulates all state and rendering logic for displaying array data,
//! including pan/zoom, stretch functions, colormaps, and overlays. Multiple instances
//! can be used side-by-side without sharing state.

use egui::{Color32, ColorImage, Key, PointerButton, Response, TextureHandle, TextureOptions, Ui, Vec2};

use crate::colormap::Colormap;
use crate::transform::{self, ViewTransform};

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
/// Duration to show zoom level overlay after zooming
const ZOOM_OVERLAY_DURATION: f64 = 0.5;

/// Actions returned from zoom controls overlay
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoomAction {
    None,
    ZoomIn,
    ZoomOut,
    Reset,
}

/// Actions returned from stretch controls overlay
#[derive(Clone, Copy, Debug, PartialEq)]
enum StretchAction {
    None,
    SetLinear,
    SetLog,
    SetDiverging,
    SetColormap(Colormap),
    ToggleReverse,
    ResetStretch,
}

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

/// A self-contained widget for viewing 2D array/image data.
///
/// This widget owns all its state and can be embedded in any egui application.
/// Multiple instances can coexist without sharing state.
pub struct ArrayViewerWidget {
    // === Image data ===
    /// Raw pixel data as f64 values
    pixels: Option<Vec<f64>>,
    /// Image width in pixels
    width: u32,
    /// Image height in pixels
    height: u32,
    /// Computed min value for scaling
    min_val: f64,
    /// Computed max value for scaling
    max_val: f64,
    /// Whether the source data is integer-typed (for display formatting)
    is_integer: bool,

    // === View transformation ===
    /// Pan/zoom transformation state
    transform: ViewTransform,

    // === Stretch settings ===
    /// Current stretch type (Linear or Log)
    stretch_type: StretchType,
    /// Contrast/bias settings for Linear mode
    linear_cb: ContrastBias,
    /// Contrast/bias settings for Log mode
    log_cb: ContrastBias,
    /// Contrast settings for Symmetric mode (bias is ignored)
    symmetric_cb: ContrastBias,
    /// Whether user is currently dragging to adjust contrast/bias
    is_adjusting_stretch: bool,

    // === Colormap ===
    /// Current colormap for standard (Lin/Log) modes
    standard_colormap: Colormap,
    /// Current colormap for symmetric/diverging mode
    diverging_colormap: Colormap,
    /// Symmetric mode (scale around zero)
    symmetric_mode: bool,
    /// Whether colormap is reversed
    colormap_reversed: bool,

    // === Rendering state ===
    /// Flag indicating texture needs rebuild
    texture_dirty: bool,
    /// Cached hover information: (image_x, image_y, raw_value)
    hover_info: Option<(u32, u32, f64)>,
    /// Main image texture
    texture: Option<TextureHandle>,
    /// Colorbar texture
    colorbar_texture: Option<TextureHandle>,
    /// Track if right mouse button started a drag (for contrast/bias adjustment)
    stretch_drag_active: bool,
    /// Track when zoom was last changed (for overlay display)
    zoom_changed_time: Option<f64>,
    /// Previous zoom level to detect changes
    prev_zoom_level: f32,
}

impl Default for ArrayViewerWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ArrayViewerWidget {
    /// Create a new empty widget
    pub fn new() -> Self {
        Self {
            pixels: None,
            width: 0,
            height: 0,
            min_val: 0.0,
            max_val: 1.0,
            is_integer: false,
            transform: ViewTransform::new(),
            stretch_type: StretchType::default(),
            linear_cb: ContrastBias::default(),
            log_cb: ContrastBias::default(),
            symmetric_cb: ContrastBias::default(),
            is_adjusting_stretch: false,
            standard_colormap: Colormap::default(),
            diverging_colormap: Colormap::RdBu,
            symmetric_mode: false,
            colormap_reversed: false,
            texture_dirty: false,
            hover_info: None,
            texture: None,
            colorbar_texture: None,
            stretch_drag_active: false,
            zoom_changed_time: None,
            prev_zoom_level: 1.0,
        }
    }

    // =========================================================================
    // Public API (called from outside, e.g., from JS via ViewerHandle)
    // =========================================================================

    /// Set new image data, computing min/max for auto-scaling.
    /// Pan is reset if dimensions change; zoom is always preserved.
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

    /// Check if we have image data
    pub fn has_image(&self) -> bool {
        self.pixels.is_some()
    }

    /// Get image dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
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

    /// Get mutable reference to transform
    pub fn transform_mut(&mut self) -> &mut ViewTransform {
        &mut self.transform
    }

    /// Get reference to transform
    pub fn transform(&self) -> &ViewTransform {
        &self.transform
    }

    /// Check if view is at default state
    pub fn is_default_view(&self) -> bool {
        self.transform.is_default()
    }

    // =========================================================================
    // Stretch / Colormap API
    // =========================================================================

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

    /// Set stretch type directly (used by selectable labels)
    pub fn set_stretch_type(&mut self, stretch_type: StretchType) {
        if self.stretch_type != stretch_type {
            self.stretch_type = stretch_type;
            // If switching to log, disable symmetric mode (log doesn't work well with negative values)
            if stretch_type == StretchType::Log && self.symmetric_mode {
                self.symmetric_mode = false;
            }
            self.texture_dirty = true;
        }
    }

    /// Get current contrast/bias for the active stretch mode
    pub fn current_contrast_bias(&self) -> ContrastBias {
        if self.symmetric_mode {
            // In symmetric mode, always use default bias (0.5) to keep it centered
            ContrastBias {
                contrast: self.symmetric_cb.contrast,
                bias: DEFAULT_BIAS,
            }
        } else {
            match self.stretch_type {
                StretchType::Linear => self.linear_cb,
                StretchType::Log => self.log_cb,
            }
        }
    }

    /// Get mutable reference to current contrast/bias
    fn current_contrast_bias_mut(&mut self) -> &mut ContrastBias {
        if self.symmetric_mode {
            &mut self.symmetric_cb
        } else {
            match self.stretch_type {
                StretchType::Linear => &mut self.linear_cb,
                StretchType::Log => &mut self.log_cb,
            }
        }
    }

    /// Adjust contrast/bias based on mouse drag delta
    pub fn adjust_contrast_bias(&mut self, dx: f32, dy: f32, viewport_size: Vec2) {
        // Check symmetric mode before borrowing mutably
        let is_symmetric = self.symmetric_mode;
        let cb = self.current_contrast_bias_mut();

        // In symmetric mode, only adjust contrast (ignore bias to keep scaling centered)
        if !is_symmetric {
            // Map horizontal to bias (0 to 1)
            cb.bias = (cb.bias + (dx as f64) / (viewport_size.x as f64)).clamp(0.0, 1.0);
        }

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
        self.symmetric_cb = ContrastBias::default();
        self.stretch_type = StretchType::Linear;
        self.texture_dirty = true;
    }

    /// Check if current stretch mode has non-default contrast/bias
    pub fn is_stretch_modified(&self) -> bool {
        if self.symmetric_mode {
            // In symmetric mode, only contrast matters
            (self.symmetric_cb.contrast - DEFAULT_CONTRAST).abs() >= 0.001
        } else {
            !self.current_contrast_bias().is_default()
        }
    }

    /// Set whether user is currently adjusting stretch
    pub fn set_adjusting_stretch(&mut self, adjusting: bool) {
        self.is_adjusting_stretch = adjusting;
    }

    /// Check if user is currently adjusting stretch
    pub fn is_adjusting_stretch(&self) -> bool {
        self.is_adjusting_stretch
    }

    /// Get current colormap (based on current mode)
    pub fn colormap(&self) -> Colormap {
        if self.symmetric_mode {
            self.diverging_colormap
        } else {
            self.standard_colormap
        }
    }

    /// Set colormap (stores in appropriate field based on colormap type)
    pub fn set_colormap(&mut self, colormap: Colormap) {
        if colormap.is_diverging() {
            self.diverging_colormap = colormap;
        } else {
            self.standard_colormap = colormap;
        }
        self.texture_dirty = true;
    }

    /// Check if symmetric mode is enabled
    pub fn is_symmetric(&self) -> bool {
        self.symmetric_mode
    }

    /// Enable symmetric/diverging mode
    pub fn set_symmetric(&mut self, enabled: bool) {
        if self.symmetric_mode != enabled {
            self.symmetric_mode = enabled;
            self.texture_dirty = true;
        }
    }

    /// Toggle symmetric mode
    pub fn toggle_symmetric(&mut self) {
        self.set_symmetric(!self.symmetric_mode);
    }

    /// Check if colormap is reversed
    pub fn is_reversed(&self) -> bool {
        self.colormap_reversed
    }

    /// Toggle colormap reversal
    pub fn toggle_reverse(&mut self) {
        self.colormap_reversed = !self.colormap_reversed;
        self.texture_dirty = true;
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    /// Get the scaling range based on symmetric mode
    fn scaling_range(&self) -> (f64, f64) {
        if self.symmetric_mode {
            let abs_max = self.min_val.abs().max(self.max_val.abs());
            (-abs_max, abs_max)
        } else {
            (self.min_val, self.max_val)
        }
    }

    /// Get raw pixel value at image coordinates
    fn get_pixel_value(&self, x: u32, y: u32) -> Option<f64> {
        let pixels = self.pixels.as_ref()?;
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            pixels.get(idx).copied()
        } else {
            None
        }
    }

    /// Get min/max values
    pub fn value_range(&self) -> (f64, f64) {
        (self.min_val, self.max_val)
    }

    /// Check if source data is integer-typed
    pub fn is_integer(&self) -> bool {
        self.is_integer
    }

    /// Get current hover info
    pub fn hover_info(&self) -> Option<(u32, u32, f64)> {
        self.hover_info
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

    /// Build a ColorImage from the current pixel data using colormap
    fn build_color_image(&self) -> Option<ColorImage> {
        let pixels = self.pixels.as_ref()?;

        let (scale_min, scale_max) = self.scaling_range();
        let cb = self.current_contrast_bias();
        let stretch_type = self.stretch_type;
        let colormap = self.colormap();
        let reversed = self.colormap_reversed;

        let rgba: Vec<Color32> = pixels
            .iter()
            .map(|&v| {
                let mut adjusted = self.apply_full_stretch(v, scale_min, scale_max, cb, stretch_type);
                if reversed {
                    adjusted = 1.0 - adjusted;
                }
                colormap.map(adjusted)
            })
            .collect();

        Some(ColorImage {
            size: [self.width as usize, self.height as usize],
            pixels: rgba,
        })
    }

    /// Rebuild the main image texture
    fn rebuild_texture(&mut self, ctx: &egui::Context) {
        if let Some(color_image) = self.build_color_image() {
            self.texture = Some(ctx.load_texture(
                "image",
                color_image,
                TextureOptions::NEAREST,
            ));
        }
        // Also rebuild colorbar
        self.rebuild_colorbar_texture(ctx);
    }

    /// Rebuild the colorbar texture
    fn rebuild_colorbar_texture(&mut self, ctx: &egui::Context) {
        let height = 256;
        let width = 1;

        let cb = self.current_contrast_bias();
        let stretch_type = self.stretch_type;
        let colormap = self.colormap();
        let reversed = self.colormap_reversed;

        let pixels: Vec<Color32> = (0..height)
            .rev() // Reverse so high values are at top
            .map(|y| {
                let t = y as f64 / (height - 1) as f64;
                let stretched = apply_stretch(t, stretch_type);
                let mut adjusted = apply_contrast_bias(stretched, cb.contrast, cb.bias);
                if reversed {
                    adjusted = 1.0 - adjusted;
                }
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

    // =========================================================================
    // Main rendering - called via egui::Widget trait
    // =========================================================================

    /// Show the widget, rendering into the given UI with a specified container size.
    ///
    /// The container_size determines how large the widget should render itself.
    /// This is typically the available space from the parent layout or window.
    pub fn show(&mut self, ui: &mut Ui, container_size: Vec2) -> Response {
        let ctx = ui.ctx().clone();

        // Check if texture needs rebuilding
        if self.texture_dirty {
            self.texture_dirty = false;
            self.rebuild_texture(&ctx);
        }

        // Handle keyboard shortcuts
        self.handle_keyboard_input(&ctx);

        // Allocate space for the widget
        let (rect, response) = ui.allocate_exact_size(container_size, egui::Sense::click_and_drag());

        if !self.has_image() {
            // Draw "no image" message
            let painter = ui.painter_at(rect);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No image loaded",
                egui::FontId::default(),
                ui.style().visuals.text_color(),
            );
            return response;
        }

        let (img_width, img_height) = self.dimensions();

        // Calculate base image display size (fit-to-view size)
        let available_size = container_size;
        let img_aspect = img_width as f32 / img_height as f32;
        let available_aspect = available_size.x / available_size.y;

        let base_display_size = if img_aspect > available_aspect {
            egui::vec2(available_size.x, available_size.x / img_aspect)
        } else {
            egui::vec2(available_size.y * img_aspect, available_size.y)
        };

        // Calculate the actual image rect with zoom and pan applied
        let viewport_rect = rect;
        let image_rect = self.transform.calculate_image_rect(viewport_rect, base_display_size);
        let viewport_center = viewport_rect.center();

        // Draw the image
        if let Some(texture) = &self.texture {
            let painter = ui.painter_at(rect);
            // Flip Y-axis for FITS convention: Y=0 at bottom
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
                    self.transform.zoom_around_point(zoom_delta, pointer_pos, viewport_center);
                }
            }
        }

        // Handle scroll wheel (for zoom when not using native zoom gesture)
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);
        if scroll_delta.y != 0.0 && zoom_delta == 1.0 {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
                if response.rect.contains(pointer_pos) {
                    let zoom_factor = if scroll_delta.y > 0.0 {
                        transform::SCROLL_ZOOM_STEP
                    } else {
                        1.0 / transform::SCROLL_ZOOM_STEP
                    };
                    self.transform.zoom_around_point(zoom_factor, pointer_pos, viewport_center);
                }
            }
        }

        // Handle pan via drag
        let should_pan = response.dragged_by(PointerButton::Primary)
            || response.dragged_by(PointerButton::Middle);

        if should_pan {
            let drag_delta = response.drag_delta();
            if drag_delta != Vec2::ZERO {
                self.transform.pan_by(drag_delta);
            }
        }

        // Handle contrast/bias adjustment via right-click drag (DS9 style)
        if response.drag_started_by(PointerButton::Secondary) {
            self.stretch_drag_active = true;
            self.is_adjusting_stretch = true;
        }

        if self.stretch_drag_active && response.dragged_by(PointerButton::Secondary) {
            let drag_delta = response.drag_delta();
            if drag_delta != Vec2::ZERO {
                self.adjust_contrast_bias(drag_delta.x, drag_delta.y, available_size);
            }
        }

        if response.drag_stopped_by(PointerButton::Secondary) {
            self.stretch_drag_active = false;
            self.is_adjusting_stretch = false;
        }

        // Handle alt+click to center on point
        let modifiers = ui.input(|i| i.modifiers);
        if response.clicked() && modifiers.alt {
            if let Some(click_pos) = response.interact_pointer_pos() {
                if let Some((img_x, img_y)) = self.transform.screen_to_image(
                    click_pos,
                    image_rect,
                    (img_width, img_height),
                ) {
                    self.transform.center_on_image_point(
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
            if let Some((img_x, img_y)) = self.transform.screen_to_image(
                hover_pos,
                image_rect,
                (img_width, img_height),
            ) {
                if let Some(value) = self.get_pixel_value(img_x, img_y) {
                    self.hover_info = Some((img_x, img_y, value));
                } else {
                    self.hover_info = None;
                }
            } else {
                self.hover_info = None;
            }
        } else {
            self.hover_info = None;
        }

        // Track zoom changes for overlay display
        let current_zoom = self.zoom_level();
        let current_time = ctx.input(|i| i.time);
        if (current_zoom - self.prev_zoom_level).abs() > 0.001 {
            self.zoom_changed_time = Some(current_time);
            self.prev_zoom_level = current_zoom;
        }

        // Render overlays using Areas (they render at screen coordinates)
        // We collect actions from overlays and apply them after rendering
        let zoom_action = self.render_zoom_controls(&ctx, viewport_center, rect);
        let stretch_action = self.render_stretch_controls(&ctx, rect);
        self.render_colorbar(&ctx, rect);
        self.render_stretch_info_overlay(&ctx, rect);
        self.render_zoom_info_overlay(&ctx, rect, current_time);
        self.render_hover_overlay(&ctx, rect);
        self.render_build_info(&ctx, rect);

        // Apply collected actions
        match zoom_action {
            ZoomAction::None => {}
            ZoomAction::ZoomIn => self.zoom_in(None, viewport_center),
            ZoomAction::ZoomOut => self.zoom_out(None, viewport_center),
            ZoomAction::Reset => self.zoom_to_fit(),
        }

        match stretch_action {
            StretchAction::None => {}
            StretchAction::SetLinear => {
                self.set_symmetric(false);
                self.set_stretch_type(StretchType::Linear);
            }
            StretchAction::SetLog => {
                self.set_symmetric(false);
                self.set_stretch_type(StretchType::Log);
            }
            StretchAction::SetDiverging => {
                self.set_stretch_type(StretchType::Linear);
                self.set_symmetric(true);
            }
            StretchAction::SetColormap(cmap) => self.set_colormap(cmap),
            StretchAction::ToggleReverse => self.toggle_reverse(),
            StretchAction::ResetStretch => self.reset_current_stretch(),
        }

        response
    }

    /// Handle keyboard shortcuts for zoom
    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        let viewport_center = ctx.screen_rect().center();

        ctx.input(|i| {
            // Zoom in: = or + (numpad)
            if i.key_pressed(Key::Equals) || i.key_pressed(Key::Plus) {
                self.zoom_in(None, viewport_center);
            }
            // Zoom out: - (minus)
            if i.key_pressed(Key::Minus) {
                self.zoom_out(None, viewport_center);
            }
            // Reset: 0
            if i.key_pressed(Key::Num0) {
                self.zoom_to_fit();
            }
        });
    }

    /// Render zoom control buttons at bottom-right of widget.
    /// Returns an action to be applied after rendering.
    fn render_zoom_controls(&self, ctx: &egui::Context, _viewport_center: egui::Pos2, widget_rect: egui::Rect) -> ZoomAction {
        let button_size = egui::vec2(28.0, 28.0);
        let margin = 10.0;
        let spacing = 4.0;

        let num_buttons = 3.0;
        let base_x = widget_rect.max.x - margin - button_size.x * num_buttons - spacing * (num_buttons - 1.0);
        let base_y = widget_rect.max.y - margin - button_size.y;

        let mut action = ZoomAction::None;

        egui::Area::new(egui::Id::new("zoom_controls"))
            .fixed_pos(egui::pos2(base_x, base_y))
            .show(ctx, |ui| {
                // Get themed colors
                let frame_style = overlay_frame(ui);
                let text_color = get_overlay_text_color(ui);

                frame_style.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = spacing;

                        // Always show reset button, but disable when at default view
                        let can_reset = !self.is_default_view();
                        let reset_color = if can_reset { text_color } else { text_color.gamma_multiply(0.3) };
                        let reset_btn = egui::Button::new(
                            egui::RichText::new("⟲").color(reset_color)
                        ).fill(Color32::TRANSPARENT);
                        let reset_response = ui.add_sized(button_size, reset_btn);
                        if can_reset && reset_response.clicked() {
                            action = ZoomAction::Reset;
                        }

                        let minus_btn = egui::Button::new(
                            egui::RichText::new("−").color(text_color)
                        ).fill(Color32::TRANSPARENT);
                        if ui.add_sized(button_size, minus_btn).clicked() {
                            action = ZoomAction::ZoomOut;
                        }

                        let plus_btn = egui::Button::new(
                            egui::RichText::new("+").color(text_color)
                        ).fill(Color32::TRANSPARENT);
                        if ui.add_sized(button_size, plus_btn).clicked() {
                            action = ZoomAction::ZoomIn;
                        }
                    });
                });
            });

        action
    }

    /// Render stretch controls at top-right of widget.
    /// Returns an action to be applied after rendering.
    fn render_stretch_controls(&self, ctx: &egui::Context, _widget_rect: egui::Rect) -> StretchAction {
        let margin = 10.0;

        let mut action = StretchAction::None;

        egui::Area::new(egui::Id::new("stretch_controls"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-margin, margin))
            .show(ctx, |ui| {
                let stretch_type = self.stretch_type();
                let is_modified = self.is_stretch_modified();
                let colormap = self.colormap();
                let symmetric = self.is_symmetric();
                let reversed = self.is_reversed();

                // Get themed colors for overlay
                let frame_style = overlay_frame(ui);
                let text_color = get_overlay_text_color(ui);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Reset button on far left (only shown when modified) - same style as other buttons
                    if is_modified {
                        frame_style.show(ui, |ui| {
                            let btn = egui::Button::new(
                                egui::RichText::new("⟲").color(text_color)
                            ).fill(Color32::TRANSPARENT);
                            if ui.add(btn).on_hover_text("Reset contrast/bias").clicked() {
                                action = StretchAction::ResetStretch;
                            }
                        });
                    }

                    // Colormaps group with Rev toggle
                    frame_style.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if symmetric {
                                // Diverging colormaps for symmetric mode
                                for &cmap in Colormap::diverging_colormaps() {
                                    let selected = colormap == cmap;
                                    let label = egui::RichText::new(cmap.name()).color(text_color);
                                    if ui.selectable_label(selected, label).clicked() {
                                        action = StretchAction::SetColormap(cmap);
                                    }
                                }
                            } else {
                                // Standard colormaps for Lin/Log modes
                                for &cmap in Colormap::standard_colormaps() {
                                    let selected = colormap == cmap;
                                    let label = egui::RichText::new(cmap.name()).color(text_color);
                                    if ui.selectable_label(selected, label).clicked() {
                                        action = StretchAction::SetColormap(cmap);
                                    }
                                }
                            }

                            ui.separator();

                            // Reverse toggle
                            let rev_label = egui::RichText::new("Rev").color(text_color);
                            if ui.selectable_label(reversed, rev_label).on_hover_text("Reverse colormap").clicked() {
                                action = StretchAction::ToggleReverse;
                            }
                        });
                    });

                    // Stretch modes group
                    frame_style.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let lin_label = egui::RichText::new("Lin").color(text_color);
                            if ui.selectable_label(stretch_type == StretchType::Linear && !symmetric, lin_label).clicked() {
                                action = StretchAction::SetLinear;
                            }
                            let log_label = egui::RichText::new("Log").color(text_color);
                            if ui.selectable_label(stretch_type == StretchType::Log, log_label).clicked() {
                                action = StretchAction::SetLog;
                            }
                            let div_label = egui::RichText::new("±").color(text_color);
                            if ui.selectable_label(symmetric, div_label).on_hover_text("Symmetric scaling (diverging)").clicked() {
                                action = StretchAction::SetDiverging;
                            }
                        });
                    });
                });
            });

        action
    }

    /// Render colorbar overlay at top-left of widget
    fn render_colorbar(&self, ctx: &egui::Context, widget_rect: egui::Rect) {
        if !self.has_image() {
            return;
        }

        let (scale_min, scale_max) = self.scaling_range();
        let is_int = self.is_integer();

        let bar_height = COLORBAR_MAX_HEIGHT.min(widget_rect.height() * 0.5);
        let bar_width = COLORBAR_WIDTH;

        // Translucent background for labels
        let label_bg = egui::Color32::from_black_alpha(140);

        egui::Area::new(egui::Id::new("colorbar"))
            .fixed_pos(egui::pos2(widget_rect.min.x + COLORBAR_MARGIN, widget_rect.min.y + COLORBAR_MARGIN))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let max_label = if is_int {
                        format!("{}", scale_max as i64)
                    } else {
                        format_scientific(scale_max)
                    };
                    egui::Frame::none()
                        .fill(label_bg)
                        .rounding(2.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(&max_label).color(Color32::WHITE).small());
                        });

                    if let Some(texture) = &self.colorbar_texture {
                        let bar_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(bar_width, bar_height),
                        );
                        ui.allocate_space(egui::vec2(bar_width, bar_height));

                        let painter = ui.painter();
                        painter.rect_stroke(
                            bar_rect.expand(1.0),
                            0.0,
                            egui::Stroke::new(1.0, Color32::GRAY),
                        );
                        painter.image(
                            texture.id(),
                            bar_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }

                    let min_label = if is_int {
                        format!("{}", scale_min as i64)
                    } else {
                        format_scientific(scale_min)
                    };
                    egui::Frame::none()
                        .fill(label_bg)
                        .rounding(2.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(&min_label).color(Color32::WHITE).small());
                        });
                });
            });
    }

    /// Render contrast/bias values while adjusting
    fn render_stretch_info_overlay(&self, ctx: &egui::Context, widget_rect: egui::Rect) {
        if !self.is_adjusting_stretch() {
            return;
        }

        let cb = self.current_contrast_bias();
        let stretch_type = self.stretch_type();

        egui::Area::new(egui::Id::new("stretch_info_overlay"))
            .fixed_pos(egui::pos2(widget_rect.center().x - 80.0, widget_rect.min.y + 50.0))
            .show(ctx, |ui| {
                let text_color = get_overlay_text_color(ui);
                let bg = get_overlay_bg(ui);
                egui::Frame::popup(ui.style())
                    .fill(bg)
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
                            .color(text_color),
                        );
                    });
            });
    }

    /// Render zoom level overlay while zooming (similar to contrast adjustment overlay)
    fn render_zoom_info_overlay(&self, ctx: &egui::Context, widget_rect: egui::Rect, current_time: f64) {
        // Check if we should show the overlay (during and shortly after zoom changes)
        let should_show = if let Some(changed_time) = self.zoom_changed_time {
            (current_time - changed_time) < ZOOM_OVERLAY_DURATION
        } else {
            false
        };

        if !should_show {
            return;
        }

        let zoom_level = self.zoom_level();
        let zoom_text = format_zoom_multiple(zoom_level);

        egui::Area::new(egui::Id::new("zoom_info_overlay"))
            .fixed_pos(egui::pos2(widget_rect.center().x - 50.0, widget_rect.center().y - 20.0))
            .show(ctx, |ui| {
                let text_color = get_overlay_text_color(ui);
                let bg = get_overlay_bg(ui);
                egui::Frame::popup(ui.style())
                    .fill(bg)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.label(
                            egui::RichText::new(zoom_text)
                                .color(text_color)
                                .size(24.0),
                        );
                    });
            });
    }

    /// Render build info at bottom-left of widget
    fn render_build_info(&self, ctx: &egui::Context, widget_rect: egui::Rect) {
        let margin = 10.0;

        egui::Area::new(egui::Id::new("build_info"))
            .fixed_pos(egui::pos2(widget_rect.min.x + margin, widget_rect.max.y - margin - 20.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(env!("BUILD_TIMESTAMP"))
                        .color(egui::Color32::from_white_alpha(80))
                        .small(),
                );
            });
    }

    /// Render hover info overlay at bottom-left of widget
    fn render_hover_overlay(&self, ctx: &egui::Context, widget_rect: egui::Rect) {
        if let Some((x, y, value)) = self.hover_info() {
            let is_int = self.is_integer();

            egui::Area::new(egui::Id::new("hover_overlay"))
                .fixed_pos(egui::pos2(widget_rect.min.x + 10.0, widget_rect.max.y - 30.0))
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

/// Apply stretch function to a normalized value (0-1)
fn apply_stretch(x: f64, stretch_type: StretchType) -> f64 {
    match stretch_type {
        StretchType::Linear => x,
        StretchType::Log => {
            (LOG_EXPONENT * x + 1.0).log10() / LOG_EXPONENT.log10()
        }
    }
}

/// Apply DS9-style contrast/bias transformation
fn apply_contrast_bias(x: f64, contrast: f64, bias: f64) -> f64 {
    ((x - bias) * contrast + 0.5).clamp(0.0, 1.0)
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

/// Get a translucent background color appropriate for light/dark mode
fn get_overlay_bg(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_black_alpha(180)
    } else {
        Color32::from_white_alpha(220)
    }
}

/// Get text color appropriate for light/dark mode overlays
fn get_overlay_text_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::WHITE
    } else {
        Color32::from_gray(30)
    }
}

/// Create a frame style for overlay controls that adapts to light/dark mode
fn overlay_frame(ui: &Ui) -> egui::Frame {
    let bg = get_overlay_bg(ui);
    egui::Frame::none()
        .fill(bg)
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
}

/// Format zoom level as a nice multiple string with consistent decimal places
fn format_zoom_multiple(zoom: f32) -> String {
    format!("{:.3}x", zoom)
}
