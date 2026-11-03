//! Check the server message before its native handler reads it.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, MessageCheckOnMessageExit};
use crate::vostok::match_message::Message;
use crate::vostok::log::{line, Piece};

pub fn on_message(ctx: &mut Context<'_>) -> Flow<MessageCheckOnMessageExit> {
    let Some(message) = Message::at_dispatch(ctx) else {
        // These registers are native objects, not packet fields. A null
        // object cannot arise at the dispatch site; never enter a reader.
        return Flow::Exit(MessageCheckOnMessageExit::Refused);
    };
    if message.is_valid() { return Flow::Continue; }
    line(&[Piece::Text("message_check: refused message "), Piece::Hex(u32::from(message.id())),
           Piece::Text("; the match left")]);
    message.leave();
    Flow::Exit(MessageCheckOnMessageExit::Refused)
}
