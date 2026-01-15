//! Thin application shell for the array viewer
//!
//! This module contains the eframe App implementation that hosts an ArrayViewerWidget.
//! The app is responsible for:
//! - Hosting the widget in a CentralPanel
//! - Passing available size from egui's layout to the widget
//! - Continuous repaint requests for smooth updates

use std::cell::RefCell;
use std::rc::Rc;

use crate::widget::ArrayViewerWidget;

/// The eframe application shell for the viewer.
///
/// This is a thin wrapper that hosts a single ArrayViewerWidget and
/// manages the application lifecycle. The widget itself contains all
/// viewing state and rendering logic.
pub struct ViewerApp {
    /// The widget instance (shared with ViewerHandle for external control)
    widget: Rc<RefCell<ArrayViewerWidget>>,
}

impl ViewerApp {
    /// Create a new application with the given widget instance.
    ///
    /// The widget is shared via Rc<RefCell<>> so that ViewerHandle can
    /// call methods on it from JavaScript.
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        widget: Rc<RefCell<ArrayViewerWidget>>,
    ) -> Self {
        Self { widget }
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Use a CentralPanel with no margin/padding
        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(0.0);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // Use the actual available size from egui's layout system
            let container_size = ui.available_size();
            
            // Render the widget
            let mut widget = self.widget.borrow_mut();
            widget.show(ui, container_size);
        });

        // Request continuous repaints for smooth updates
        ctx.request_repaint();
    }
}
