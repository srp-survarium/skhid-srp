use vostok::serde::advance_buffer;
use vostok::serde::DeserializeError;

use raw::*;

/// 0.20e
pub enum ServerGameMessageKind {
    // ConnectionSuccessful,
    // MatchOptions {
    //     map_id: u8,
    //     // 32 chars max
    //     map_name: String,
    //     match_mode: game_mode_type,
    //     player_count: u8,
    //     victory_item_count: u8,
    //     respawn_time: u8,
    //     match_time: u16,
    // },
    // #[expect(dead_code)]
    // MatchTimeChanged {
    //     match_time: u32,
    // },
    // PlayerProfile {
    //     player_profile: Box<player_profile>,
    // },
    ConnectionSuccessful {
        first_port: u16,
        second_port: u16,
    },
    StaticMatchInfo {
        map_id: u8,
        match_mode: u8,
        player_count: u8,
        victory_item_count: u8,
        respawn_time: u8,
        match_time: u16,
        match_id: u32,
        wait_player_percent: f32,
        wait1_time: u8,
        wait2_time: u8,
        countdown_time: u8,
        event_scores: [u16; 21],
        players: Vec<u8>,
    },
    SetTimeRequest {},
    DynamicMatchInfo {},
    PlayerInputChange {},
    FloatingTimerSetOffset {},
    PlayerEnteredMatch {},
    PlayerLeftMatch {},
    LocalPlayerInputDiscard {},
    OnHashMismatch {},
}

pub mod raw {
    #![expect(non_camel_case_types)]
    #![expect(dead_code)]

    #[repr(u8)]
    #[derive(bytemuck::CheckedBitPattern, bytemuck::NoUninit, Copy, Clone, Debug, PartialEq)]
    #[rustfmt::skip]
    pub enum match_server_message_types_enum {
        match_server_connection_successful = 0x50,
        static_match_info                  = 0x51,
        set_time_request                   = 0x52,
        dynamic_match_info                 = 0x53,
        player_input_change                = 0x54,
        floating_timer_set_offset          = 0x55,
        player_entered_match               = 0x56,
        player_left_match                  = 0x57,
        local_player_input_discard         = 0x58,
        on_hash_mismatch                   = 0x59,
    }

    #[repr(u8)]
    #[derive(bytemuck::CheckedBitPattern, bytemuck::NoUninit, Copy, Clone, Debug, PartialEq)]
    #[rustfmt::skip]
    pub enum game_mode_type {
        capture_enemy_base   = 0x0,
        capture_neutral_base = 0x1,
        gather_victory_items = 0x2,
        invalid_game_mode    = 0xFF,
    }
}

impl ServerGameMessageKind {
    #[rustfmt::skip]
    pub fn message_type(&self) -> match_server_message_types_enum {
        match self {
            Self::ConnectionSuccessful { .. }    => match_server_message_types_enum::match_server_connection_successful,
            Self::StaticMatchInfo { .. }         => match_server_message_types_enum::static_match_info,
            Self::SetTimeRequest { .. }          => match_server_message_types_enum::set_time_request,
            Self::DynamicMatchInfo { .. }        => match_server_message_types_enum::dynamic_match_info,
            Self::PlayerInputChange { .. }       => match_server_message_types_enum::player_input_change,
            Self::FloatingTimerSetOffset { .. }  => match_server_message_types_enum::floating_timer_set_offset,
            Self::PlayerEnteredMatch { .. }      => match_server_message_types_enum::player_entered_match,
            Self::PlayerLeftMatch { .. }         => match_server_message_types_enum::player_left_match,
            Self::LocalPlayerInputDiscard { .. } => match_server_message_types_enum::local_player_input_discard,
            Self::OnHashMismatch { .. }          => match_server_message_types_enum::on_hash_mismatch,
        }
    }
}

impl ServerGameMessageKind {
    fn deserialize(buffer: &mut &[u8]) -> Result<Self, DeserializeError> {
        let msg_type = advance_buffer::<match_server_message_types_enum>(buffer)?;

        use match_server_message_types_enum::*;
        let result = match msg_type {
            match_server_connection_successful => {
                let first_port = advance_buffer::<u16>(buffer)?;
                let second_port = advance_buffer::<u16>(buffer)?;

                Self::ConnectionSuccessful {
                    first_port,
                    second_port,
                }
            }

            static_match_info => {
                let map_id = advance_buffer::<u8>(buffer)?;
                let match_mode = advance_buffer::<u8>(buffer)?;
                let player_count = advance_buffer::<u8>(buffer)?;
                let victory_item_count = advance_buffer::<u8>(buffer)?;
                let respawn_time = advance_buffer::<u8>(buffer)?;
                let match_time = advance_buffer::<u16>(buffer)?;
                let match_id = advance_buffer::<u32>(buffer)?;
                let wait_player_percent = advance_buffer::<f32>(buffer)?;
                let wait1_time = advance_buffer::<u8>(buffer)?;
                let wait2_time = advance_buffer::<u8>(buffer)?;
                let countdown_time = advance_buffer::<u8>(buffer)?;

                let mut event_scores = [0_u16; 21];
                for event_score in &mut event_scores {
                    *event_score = advance_buffer::<u16>(buffer)?;
                }

                let mut players = vec![0_u8; player_count as usize];
                for i in 0..player_count {
                    let player = advance_buffer::<u8>(buffer)?;
                    players[i as usize] = player;
                }

                Self::StaticMatchInfo {
                    map_id,
                    match_mode,
                    player_count,
                    victory_item_count,
                    respawn_time,
                    match_time,
                    match_id,
                    wait_player_percent,
                    wait1_time,
                    wait2_time,
                    countdown_time,
                    event_scores,
                    players,
                }
            }

            set_time_request => todo!(),

            dynamic_match_info => todo!(),
            player_input_change => todo!(),
            floating_timer_set_offset => todo!(),
            player_entered_match => todo!(),
            player_left_match => todo!(),
            local_player_input_discard => todo!(),
            on_hash_mismatch => todo!(),
        };
        Ok(result)
    }

    // fn serialize(self, packet: &mut impl Packet) {
    //     // Note that tag is written before `order_id`, which is not part of the message
    //     match self {
    //         Self::ConnectionSuccessful {} => (),
    //         Self::MatchOptions {
    //             map_id,
    //             map_name,
    //             match_mode,
    //             player_count,
    //             victory_item_count,
    //             respawn_time,
    //             match_time,
    //         } => {
    //             packet.write(map_id);
    //             assert!(map_name.len() < 30);
    //             packet.write_str(&map_name);
    //             packet.write(match_mode);
    //             packet.write(player_count);
    //             packet.write(victory_item_count);
    //             packet.write(respawn_time);
    //             packet.write(match_time);
    //         }
    //         Self::MatchTimeChanged { match_time } => {
    //             packet.write(match_time);
    //         }
    //         Self::PlayerProfile { player_profile } => {
    //             player_profile.serialize_udp(packet);
    //         }
    //     }
    // }
}
