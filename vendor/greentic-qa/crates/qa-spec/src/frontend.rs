use serde_json::Value;

use crate::{RenderPayload, render_card, render_json_ui, render_text};

/// Abstraction over UI frontends that render the same payload into different transports.
pub trait QaFrontend {
    fn render_text_ui(&self, payload: &RenderPayload) -> String;
    fn render_json_ui(&self, payload: &RenderPayload) -> Value;
    fn render_adaptive_card(&self, payload: &RenderPayload) -> Value;
}

/// Default frontend implementation that reuses existing renderer functions.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultQaFrontend;

impl QaFrontend for DefaultQaFrontend {
    fn render_text_ui(&self, payload: &RenderPayload) -> String {
        render_text(payload)
    }

    fn render_json_ui(&self, payload: &RenderPayload) -> Value {
        render_json_ui(payload)
    }

    fn render_adaptive_card(&self, payload: &RenderPayload) -> Value {
        render_card(payload)
    }
}
