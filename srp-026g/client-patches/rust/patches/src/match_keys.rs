//! Patch 58: in a match the keys 1, 2 and 3 pick kit 1, 2 and 3 while the
//! kits are shown, and an option hides the other players' chat lines
//! (client_patch.md section 3.53; the rules: docs/notes/match-hud-options.md).
//!
//! The keys: a key 1, 2 or 3 going down while the hud shows the cursor (as
//! it does with the kits) calls the hud movie's `root.select_kit_key(0, 1
//! or 2)`, which picks that kit as a click would when the kits are shown and
//! the chat takes no typing (HUD.as). The key is not taken: the game world's
//! function goes on as before, and so does the player's handler.
//!
//! The chat: the gameplay options' "g_private_messages_in_game" names no
//! console variable in the original, so the option is dead. Once the hud is
//! made, a cc_bool of that name is registered on HIDE_CHAT (kept in
//! user.cfg): the option is live (its label "Hide chat in match", content
//! edit match-chat-off-1.json). With it on, in a match, a received line of a
//! players' channel the hud shows is dropped; the server's lines, the general
//! and private channels and the player's own lines (the local echo, another
//! call) are shown as before.

#![forbid(unsafe_code)]

use crate::vostok::console::{ConsoleBool, OptionName};
use crate::vostok::world_input::{KeyKind, WorldKey};
use crate::vostok::{Channel, ChatHandler, ChatText, Context, Flow, NoExit, OrigAddMessage};

/// The match chat option's command and byte (1 hides the lines).
static HIDE_CHAT: ConsoleBool = ConsoleBool::new();

/// `world_keyboard.kind_test`: a key reaches the game world.
pub fn on_kind_test(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let key = WorldKey::at(ctx);
    if key.kind() == KeyKind::Down
        && let Some(digit @ 1..=3) = key.key().digit()
        && key.cursor_shown()
        && let Some(movie) = key.hud_movie()
    {
        movie.invoke_uint(c"root.select_kit_key", digit - 1);
    }
    Flow::Continue
}

/// `world.hud_made`: the game world's hud was made; the option's variable
/// is registered, once.
pub fn on_hud_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    HIDE_CHAT.make_once(ctx.thread(), OptionName::PrivateMessagesInGame);
    Flow::Continue
}

/// `chat.add_message`: a received line, dropped with the option on, in a
/// match, on a channel the hud shows.
pub fn on_add_message(add: &OrigAddMessage, handler: &mut ChatHandler, channel: Channel, body: ChatText,
                      name: ChatText, tag: ChatText) {
    if HIDE_CHAT.get(handler.thread()) && handler.in_match() && channel.shown_in_match() {
        return;
    }
    add.call(handler, channel, body, name, tag);
}
