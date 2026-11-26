//! A file check (patch 215, `srp_validate::checks`): the game tells the
//! match server the `resources.db.patch_<n>` it mounted, and, when the
//! server asks, the size and sha256 of each file it names as the game
//! mounts it: the copy in the last archive that has it.
#![forbid(unsafe_code)]

pub mod table;
pub mod run;
