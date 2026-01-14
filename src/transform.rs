//! Coordinate transformation logic for pan/zoom functionality
//!
//! This module contains pure coordinate transformation logic that can be
//! easily unit tested without egui dependencies.

use egui::{Pos2, Rect, Vec2};

/// Zoom step multiplier for zoom in/out operations (buttons/keyboard)
pub const ZOOM_STEP: f32 = 1.25;

/// Zoom step multiplier for scroll wheel (smaller for finer control)
pub const SCROLL_ZOOM_STEP: f32 = 1.08;

/// Minimum zoom level (10% of fit-to-view)
pub const MIN_ZOOM: f32 = 0.1;

/// Maximum zoom level (5000% of fit-to-view)
pub const MAX_ZOOM: f32 = 50.0;

/// View transformation state for pan and zoom
#[derive(Clone, Debug)]
pub struct ViewTransform {
    /// Zoom level: 1.0 = fit-to-view, >1 = zoomed in, <1 = zoomed out
    pub zoom: f32,
    /// Pan offset in screen coordinates (pixels)
    pub pan_offset: Vec2,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
        }
    }
}

impl ViewTransform {
    /// Create a new transform at default zoom (fit-to-view) with no pan
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset to fit-to-view state (both zoom and pan)
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
    }

    /// Reset only pan offset, keeping current zoom level
    pub fn reset_pan(&mut self) {
        self.pan_offset = Vec2::ZERO;
    }

    /// Check if transform is at default state (for showing/hiding reset button)
    pub fn is_default(&self) -> bool {
        (self.zoom - 1.0).abs() < 0.001 && self.pan_offset.length() < 0.5
    }

    /// Zoom in by one step, centered on the given screen position
    pub fn zoom_in(&mut self, center: Option<Pos2>, viewport_center: Pos2) {
        let center = center.unwrap_or(viewport_center);
        self.zoom_around_point(ZOOM_STEP, center, viewport_center);
    }

    /// Zoom out by one step, centered on the given screen position
    pub fn zoom_out(&mut self, center: Option<Pos2>, viewport_center: Pos2) {
        let center = center.unwrap_or(viewport_center);
        self.zoom_around_point(1.0 / ZOOM_STEP, center, viewport_center);
    }

    /// Apply a zoom delta centered on a specific screen position.
    /// This preserves the point under the cursor while zooming.
    /// 
    /// The math: pan_offset is defined relative to viewport_center (see calculate_image_rect).
    /// To keep screen_pos showing the same image content after zoom:
    ///   pan_new = (screen_pos - viewport_center) * (1 - zoom_ratio) + pan_old * zoom_ratio
    pub fn zoom_around_point(&mut self, zoom_delta: f32, screen_pos: Pos2, viewport_center: Pos2) {
        if zoom_delta == 1.0 {
            return;
        }

        let old_zoom = self.zoom;
        let new_zoom = (old_zoom * zoom_delta).clamp(MIN_ZOOM, MAX_ZOOM);

        if (new_zoom - old_zoom).abs() < 0.0001 {
            return; // No change after clamping
        }

        let zoom_ratio = new_zoom / old_zoom;
        
        // d = position relative to viewport center
        let d = screen_pos - viewport_center;
        
        // New pan offset to keep the point under cursor fixed
        self.pan_offset = d * (1.0 - zoom_ratio) + self.pan_offset * zoom_ratio;
        self.zoom = new_zoom;
    }

    /// Apply a pan delta (in screen coordinates)
    pub fn pan_by(&mut self, delta: Vec2) {
        self.pan_offset += delta;
    }

    /// Center the view on a specific image position
    pub fn center_on_image_point(
        &mut self,
        image_pos: Pos2,
        image_size: Vec2,
        viewport_size: Vec2,
        base_image_rect: Rect,
    ) {
        // Calculate where this image point would be in screen coords at current zoom
        let rel_x = image_pos.x / image_size.x;
        let rel_y = image_pos.y / image_size.y;

        // Position within the zoomed image
        let zoomed_size = base_image_rect.size() * self.zoom;
        let image_screen_pos = Vec2::new(rel_x * zoomed_size.x, rel_y * zoomed_size.y);

        // We want this point to be at viewport center
        let viewport_center = viewport_size / 2.0;

        // Calculate the required offset
        let zoomed_center_offset = (viewport_size - zoomed_size) / 2.0;

        self.pan_offset = viewport_center - image_screen_pos - zoomed_center_offset;
    }

    /// Calculate the display rect for the image given viewport and base image sizes.
    /// Returns the rect where the image should be drawn in screen coordinates.
    pub fn calculate_image_rect(&self, viewport_rect: Rect, base_display_size: Vec2) -> Rect {
        let zoomed_size = base_display_size * self.zoom;

        // Base position centers the image in the viewport
        let base_offset = (viewport_rect.size() - base_display_size) / 2.0;

        // Apply zoom offset (keeping center fixed) and pan
        let zoom_offset = (base_display_size - zoomed_size) / 2.0;
        let final_offset = base_offset + zoom_offset + self.pan_offset;

        Rect::from_min_size(viewport_rect.min + final_offset, zoomed_size)
    }

    /// Convert screen position to image coordinates
    /// Note: Y is flipped for FITS convention (Y=0 at bottom of displayed image)
    pub fn screen_to_image(
        &self,
        screen_pos: Pos2,
        image_rect: Rect,
        image_size: (u32, u32),
    ) -> Option<(u32, u32)> {
        if !image_rect.contains(screen_pos) {
            return None;
        }

        let rel_x = (screen_pos.x - image_rect.min.x) / image_rect.width();
        // Flip Y: screen Y increases downward, but image Y=0 is at bottom
        let rel_y = 1.0 - (screen_pos.y - image_rect.min.y) / image_rect.height();

        let img_x = (rel_x * image_size.0 as f32).floor() as i32;
        let img_y = (rel_y * image_size.1 as f32).floor() as i32;

        if img_x >= 0 && img_x < image_size.0 as i32 && img_y >= 0 && img_y < image_size.1 as i32 {
            Some((img_x as u32, img_y as u32))
        } else {
            None
        }
    }

    /// Convert image coordinates to screen position
    /// Note: Y is flipped for FITS convention (Y=0 at bottom of displayed image)
    pub fn image_to_screen(&self, image_pos: (u32, u32), image_rect: Rect, image_size: (u32, u32)) -> Pos2 {
        let rel_x = (image_pos.0 as f32 + 0.5) / image_size.0 as f32;
        // Flip Y: image Y=0 is at bottom, but screen Y increases downward
        let rel_y = 1.0 - (image_pos.1 as f32 + 0.5) / image_size.1 as f32;

        Pos2::new(
            image_rect.min.x + rel_x * image_rect.width(),
            image_rect.min.y + rel_y * image_rect.height(),
        )
    }

    /// Clamp pan offset to keep at least part of the image visible
    pub fn clamp_pan_offset(&mut self, viewport_size: Vec2, zoomed_image_size: Vec2) {
        // Allow panning until only 10% of image is visible
        let margin = 0.1;
        let min_visible = zoomed_image_size * margin;

        // Calculate bounds for pan offset
        let max_pan_x = zoomed_image_size.x - min_visible.x;
        let max_pan_y = zoomed_image_size.y - min_visible.y;
        let min_pan_x = -(viewport_size.x - min_visible.x);
        let min_pan_y = -(viewport_size.y - min_visible.y);

        self.pan_offset.x = self.pan_offset.x.clamp(min_pan_x, max_pan_x);
        self.pan_offset.y = self.pan_offset.y.clamp(min_pan_y, max_pan_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_transform() {
        let t = ViewTransform::new();
        assert!((t.zoom - 1.0).abs() < 0.001);
        assert!(t.pan_offset.length() < 0.001);
        assert!(t.is_default());
    }

    #[test]
    fn test_reset() {
        let mut t = ViewTransform::new();
        t.zoom = 2.5;
        t.pan_offset = Vec2::new(100.0, 50.0);
        assert!(!t.is_default());

        t.reset();
        assert!(t.is_default());
    }

    #[test]
    fn test_zoom_in_out() {
        let mut t = ViewTransform::new();
        let center = Pos2::new(400.0, 300.0);

        t.zoom_in(Some(center), center);
        assert!((t.zoom - ZOOM_STEP).abs() < 0.001);

        t.zoom_out(Some(center), center);
        assert!((t.zoom - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_zoom_clamping() {
        let mut t = ViewTransform::new();
        let center = Pos2::new(400.0, 300.0);

        // Zoom way in
        for _ in 0..100 {
            t.zoom_in(Some(center), center);
        }
        assert!(t.zoom <= MAX_ZOOM);

        // Reset and zoom way out
        t.reset();
        for _ in 0..100 {
            t.zoom_out(Some(center), center);
        }
        assert!(t.zoom >= MIN_ZOOM);
    }

    #[test]
    fn test_zoom_around_point_preserves_center() {
        let mut t = ViewTransform::new();
        t.pan_offset = Vec2::new(50.0, 30.0);
        let viewport_center = Pos2::new(400.0, 300.0);
        let zoom_center = Pos2::new(200.0, 150.0);

        // Calculate what's under zoom_center before zoom
        // d = zoom_center - viewport_center
        let d = zoom_center - viewport_center;
        let old_offset = t.pan_offset;
        let old_zoom = t.zoom;

        t.zoom_around_point(2.0, zoom_center, viewport_center);

        // After zoom, the same relative position in image space should be under zoom_center
        // Using the formula: pan_new = d * (1 - zoom_ratio) + pan_old * zoom_ratio
        let zoom_ratio = 2.0;
        let expected_offset = d * (1.0 - zoom_ratio) + old_offset * zoom_ratio;

        assert!((t.pan_offset.x - expected_offset.x).abs() < 0.01);
        assert!((t.pan_offset.y - expected_offset.y).abs() < 0.01);
    }

    #[test]
    fn test_pan_by() {
        let mut t = ViewTransform::new();
        t.pan_by(Vec2::new(10.0, 20.0));
        assert!((t.pan_offset.x - 10.0).abs() < 0.001);
        assert!((t.pan_offset.y - 20.0).abs() < 0.001);

        t.pan_by(Vec2::new(-5.0, -10.0));
        assert!((t.pan_offset.x - 5.0).abs() < 0.001);
        assert!((t.pan_offset.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_screen_to_image_inside() {
        let t = ViewTransform::new();
        let image_rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 200.0));
        let image_size = (100, 100);

        // Center of image rect should map to center of image
        let center = Pos2::new(200.0, 200.0);
        let result = t.screen_to_image(center, image_rect, image_size);
        assert!(result.is_some());
        let (x, y) = result.unwrap();
        assert_eq!(x, 50);
        assert_eq!(y, 50); // Center is still center with Y-flip

        // Top-left corner of screen maps to bottom-left of image (FITS convention)
        let top_left = Pos2::new(100.0, 100.0);
        let result = t.screen_to_image(top_left, image_rect, image_size);
        assert!(result.is_some());
        let (x, y) = result.unwrap();
        assert_eq!(x, 0);
        assert_eq!(y, 99); // Y is flipped: top of screen = Y=99 in image

        // Bottom-left corner of screen maps to top-left of image (FITS convention)
        let bottom_left = Pos2::new(100.0, 299.0);
        let result = t.screen_to_image(bottom_left, image_rect, image_size);
        assert!(result.is_some());
        let (x, y) = result.unwrap();
        assert_eq!(x, 0);
        assert_eq!(y, 0); // Y is flipped: bottom of screen = Y=0 in image
    }

    #[test]
    fn test_screen_to_image_outside() {
        let t = ViewTransform::new();
        let image_rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 200.0));
        let image_size = (100, 100);

        // Outside image rect
        let outside = Pos2::new(50.0, 50.0);
        let result = t.screen_to_image(outside, image_rect, image_size);
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_image_rect_default_zoom() {
        let t = ViewTransform::new();
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let base_size = Vec2::new(400.0, 300.0);

        let result = t.calculate_image_rect(viewport, base_size);

        // Should be centered
        assert!((result.center().x - 400.0).abs() < 0.01);
        assert!((result.center().y - 300.0).abs() < 0.01);
        assert!((result.width() - 400.0).abs() < 0.01);
        assert!((result.height() - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_image_rect_zoomed() {
        let mut t = ViewTransform::new();
        t.zoom = 2.0;
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let base_size = Vec2::new(400.0, 300.0);

        let result = t.calculate_image_rect(viewport, base_size);

        // Should be 2x size, still centered
        assert!((result.width() - 800.0).abs() < 0.01);
        assert!((result.height() - 600.0).abs() < 0.01);
        assert!((result.center().x - 400.0).abs() < 0.01);
        assert!((result.center().y - 300.0).abs() < 0.01);
    }
}
