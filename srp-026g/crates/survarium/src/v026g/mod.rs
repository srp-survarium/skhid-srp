//! What srp serves the 0.26g client (`docs/notes/`).

// The game data (`data.md`, `modes_maps.md`).
pub mod armour;
pub mod awards;
pub mod dictionary;
pub mod drops;
pub mod game_data;
pub mod golden;
pub mod item_params;
pub mod loadouts;
pub mod localization;
pub mod maps;
pub mod modifications;
pub mod prices;
pub mod quests;
pub mod scopes;
pub mod shop_offers;
pub mod skills;
pub mod tables;
pub mod ui_properties;
pub mod weapon;
pub mod weapon_kit;

// Wire helpers.
pub mod lzma2;

// The lobby's messages (`lobby.md`, `lobby_requests.md`, `social.md`).
pub mod lobby;

// The match's messages and rules (`match.md`, `static_info.md`).
pub mod match_msgs;
