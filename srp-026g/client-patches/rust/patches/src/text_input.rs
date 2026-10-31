//! Ignore a character delivered to a Scaleform text field whose AS3 object
//! has already gone away. Normal input keeps the original event dispatch.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, TextInputOnCharExit, DetachedTextInput, site};

pub fn on_char(ctx: &mut Context<'_, site::TextInputCharTarget>) -> Flow<TextInputOnCharExit> {
    if let Some(input) = DetachedTextInput::at_char_event(ctx) {
        input.ignore();
        Flow::Exit(TextInputOnCharExit::Ignore)
    } else {
        Flow::Continue
    }
}
