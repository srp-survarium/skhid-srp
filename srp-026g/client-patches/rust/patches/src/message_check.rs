//! Check the server message before its native handler reads it. Every
//! discarded message is logged (`log::refusal`: at most a line a second,
//! counting the ones between).
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, MessageCheckOnMessageExit};
use crate::vostok::match_message::{Fault, Message};
use crate::vostok::log::{refusal, Piece};

pub fn on_message(ctx: &mut Context<'_>) -> Flow<MessageCheckOnMessageExit> {
    let Some(message) = Message::at_dispatch(ctx) else {
        // These registers are native objects, not packet fields. A null
        // object cannot arise at the dispatch site; never enter a reader.
        refusal(ctx.thread(), &[Piece::Text("message_check: a dispatch with no client or reader; discarded")]);
        return Flow::Exit(MessageCheckOnMessageExit::Refused);
    };
    if message.executable_reply() { return Flow::Exit(MessageCheckOnMessageExit::Refused); }
    let Some(fault) = message.fault() else { return Flow::Continue; };
    let (id, len) = (message.id(), message.payload_len().unwrap_or(0));
    message.discard();
    let (part, offset) = match fault {
        Fault::Reader => ("its reader outside its buffer", 0),
        Fault::Refused(refused) => (refused.part.name().trim_end_matches('\0'), refused.offset),
    };
    refusal(ctx.thread(), &[
        Piece::Text("message_check: message "), Piece::Hex(u32::from(id)),
        Piece::Text(" ("), Piece::Dec(len as u32), Piece::Text(" bytes) discarded: "),
        Piece::Text(part), Piece::Text(" at "), Piece::Hex(offset as u32),
    ]);
    Flow::Exit(MessageCheckOnMessageExit::Refused)
}
