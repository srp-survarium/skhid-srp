//! Dropped-weapon transport and match-owned input/presentation tracking.
//! These hooks receive/send attached targets, publish completed weapon draw
//! frames and retain original profiles at death. Local request creation and
//! gameplay transfer remain pending.
#![forbid(unsafe_code)]

use crate::vostok::pickup_input::transport;
use crate::vostok::pickup_input::{CompletedDiscard, CompletedMove, InputJournals, RetimedInput};
use crate::vostok::pickup_input::{PacketError, ReceiveError};
use crate::vostok::resources::ProfileSource;
use crate::vostok::{Context, DroppedWeaponsOnSendExit, Flow, NoExit};
use crate::vostok::{DroppedWeaponsOnPacketExit, log};
use crate::vostok::{
    DroppedWeaponsOnTransportLengthExit, DroppedWeaponsOnTransportReceivedExit,
    DroppedWeaponsOnTransportSendingExit, Reg,
};

// Storage budget, not an eviction policy: coalesced nodes may hold multiple
// targets. The ingress adapter must refuse before native mutation when full.
const TARGET_CAPACITY: usize = 4096;
static INPUTS: InputJournals<TARGET_CAPACITY> = InputJournals::new();

fn remember_dying_player(ctx: &Context<'_>) -> Flow<NoExit> {
    if let Some(source) = ProfileSource::at_player_death(ctx)
        && INPUTS.remember_profile(&source).is_err()
    {
        // A missing template must refuse later preparation; never silently
        // substitute the next occupant's equipment for this exact life.
        log::line(&[log::Piece::Text(
            "pickup: cannot retain dying player's equipment",
        )]);
    }
    Flow::Continue
}

pub fn on_player_death(ctx: &mut Context<'_>) -> Flow<NoExit> {
    remember_dying_player(ctx)
}

pub fn on_base_player_death(ctx: &mut Context<'_>) -> Flow<NoExit> {
    remember_dying_player(ctx)
}

pub fn on_constructed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.constructed(ctx);
    Flow::Continue
}

pub fn on_destroying(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.destroying(ctx);
    Flow::Continue
}

pub fn on_draw_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.begin_draw(ctx);
    Flow::Continue
}

pub fn on_draw_complete(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.complete_draw(ctx);
    Flow::Continue
}

pub fn on_weapon_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.drawn(ctx);
    Flow::Continue
}

pub fn on_local_merge(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.retimed(RetimedInput::at_local_merge(ctx));
    Flow::Continue
}

pub fn on_tick_snap(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.retimed(RetimedInput::at_tick_snap(ctx));
    Flow::Continue
}

pub fn on_moved(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.moved(CompletedMove::at_end(ctx));
    Flow::Continue
}

pub fn on_moving(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.moving(ctx);
    Flow::Continue
}

pub fn on_discarded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.discarded(CompletedDiscard::at_end(ctx));
    Flow::Continue
}

pub fn on_relay_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.end_relay(ctx);
    Flow::Continue
}

pub fn on_send(ctx: &mut Context<'_>) -> Flow<DroppedWeaponsOnSendExit> {
    if INPUTS.send(ctx) {
        Flow::Exit(DroppedWeaponsOnSendExit::Queued)
    } else {
        Flow::Continue
    }
}

pub fn on_evicting(ctx: &mut Context<'_>) -> Flow<NoExit> {
    INPUTS.evicting(ctx);
    Flow::Continue
}

pub fn on_transport_ranges(ctx: &mut Context<'_>) -> Flow<NoExit> {
    transport::extend_ranges(ctx);
    Flow::Continue
}

pub fn on_transport_length(ctx: &mut Context<'_>) -> Flow<DroppedWeaponsOnTransportLengthExit> {
    if transport::set_length(ctx) {
        Flow::Exit(DroppedWeaponsOnTransportLengthExit::Return)
    } else {
        Flow::Continue
    }
}

pub fn on_transport_sending(ctx: &mut Context<'_>) -> Flow<DroppedWeaponsOnTransportSendingExit> {
    if ctx.reg(Reg::Ecx) == transport::REQUEST_ID {
        Flow::Exit(DroppedWeaponsOnTransportSendingExit::Input)
    } else {
        Flow::Continue
    }
}

pub fn on_transport_received(ctx: &mut Context<'_>) -> Flow<DroppedWeaponsOnTransportReceivedExit> {
    if ctx.reg(Reg::Ecx) == transport::RELAY_ID {
        Flow::Exit(DroppedWeaponsOnTransportReceivedExit::Input)
    } else {
        Flow::Continue
    }
}

pub fn on_packet(ctx: &mut Context<'_>) -> Flow<DroppedWeaponsOnPacketExit> {
    let Some(outcome) = INPUTS.packet(ctx) else {
        return Flow::Continue;
    };
    if let Err(error) = outcome {
        let reason = match error {
            PacketError::Reader => "invalid native reader",
            PacketError::Payload => "invalid payload",
            PacketError::Picker => "picker outside roster",
            PacketError::Placement(error) => match error {
                ReceiveError::TimeOutsideClock => "time outside clock",
                ReceiveError::InvalidRotation => "invalid rotation",
                ReceiveError::TargetOutsideRoster => "target outside roster",
                ReceiveError::MissingInputs => "missing native input lists",
                ReceiveError::InvalidInputList => "invalid native input list",
                ReceiveError::HistoryFull => "target journal full",
                ReceiveError::ReceiveInProgress => "receive already in progress",
            },
        };
        log::line(&[
            log::Piece::Text("pickup relay rejected: "),
            log::Piece::Text(reason),
        ]);
    }
    Flow::Exit(DroppedWeaponsOnPacketExit::Processed)
}
