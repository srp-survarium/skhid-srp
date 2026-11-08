//! The faces a player may choose (`docs/notes/face-choice.md`): the
//! entries of `body_parts.heads` in `gameplay/bodyparts/default`, the file
//! the client's character cook reads (`profile_skin_visual_cook` 0x204560).
//! Face 0 is `heads[0]`, today's head; data without `heads` has that one
//! face alone (`body_parts.head`). The server keeps and sends only faces
//! the data lists: a number from a request or the database becomes a
//! [`Face`] here, by [`Faces::face`], and nowhere else.

use std::num::NonZeroU8;

use binary_config_parser::tree::{ConfigError, ConfigValue};

/// Its path in the archive.
pub const PATH: &str = "gameplay/bodyparts/default";

/// The most faces the lobby's request can choose
/// (`lobby::faces::FACE_CHOOSE_BASE` + 0 to 89).
pub const MAX_FACES: u8 = 90;

/// A face of the game data: its index in `body_parts.heads`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Face(u8);

impl Face {
    /// Today's head, everyone's until they choose.
    pub const DEFAULT: Face = Face(0);

    /// The index the wire and the database carry.
    pub fn index(self) -> u8 {
        self.0
    }
}

/// How many faces the game data lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faces {
    count: NonZeroU8,
}

impl Faces {
    /// The players' faces ([`super::game_data`]), read once. Checked at
    /// start ([`super::tables::check`]).
    pub fn shared() -> Faces {
        static SHARED: std::sync::OnceLock<Faces> = std::sync::OnceLock::new();
        *SHARED.get_or_init(|| {
            Self::of_data(super::game_data::shared()).expect("the faces read (checked at start)")
        })
    }

    /// The faces of `data`: one without the file or without `heads`.
    pub fn of_data(data: &super::game_data::GameData) -> Result<Faces, ConfigError> {
        match data.file(PATH) {
            Some(bytes) => Self::parse(bytes),
            None => Ok(Self::one()),
        }
    }

    /// The faces of the file's bytes. `heads` is an array whose every entry
    /// has `data.base_model` and `data.part_name` (what the client draws of
    /// a face, its variants falling back to `data`); a malformed one, or
    /// more than [`MAX_FACES`], stops srp at start: the client would draw
    /// faces srp could not name.
    pub fn parse(bytes: &[u8]) -> Result<Faces, ConfigError> {
        let root = ConfigValue::parse(bytes)?;
        let Some(heads) = root.req("body_parts")?.get("heads") else {
            return Ok(Self::one());
        };
        let ConfigValue::Indexed(heads) = heads else {
            return Err(ConfigError("`body_parts.heads` is no array".into()));
        };
        for (index, head) in heads.iter().enumerate() {
            let data = head
                .req("data")
                .map_err(|cause| ConfigError(format!("`body_parts.heads[{index}]`: {}", cause.0)))?;
            for key in ["base_model", "part_name"] {
                data.str_at(key).map_err(|cause| {
                    ConfigError(format!("`body_parts.heads[{index}].data`: {}", cause.0))
                })?;
            }
        }
        Self::of_count(heads.len())
    }

    /// `count` faces: 1 to [`MAX_FACES`].
    pub fn of_count(count: usize) -> Result<Faces, ConfigError> {
        u8::try_from(count)
            .ok()
            .filter(|&count| count <= MAX_FACES)
            .and_then(NonZeroU8::new)
            .map(|count| Faces { count })
            .ok_or_else(|| {
                ConfigError(format!(
                    "`body_parts.heads` has {count} faces: 1 to {MAX_FACES} can be chosen"
                ))
            })
    }

    fn one() -> Faces {
        Faces {
            count: NonZeroU8::MIN,
        }
    }

    /// How many there are.
    pub fn count(self) -> u8 {
        self.count.get()
    }

    /// The face at `index`, if the data lists it.
    pub fn face(self, index: u32) -> Option<Face> {
        u8::try_from(index)
            .ok()
            .filter(|&index| index < self.count.get())
            .map(Face)
    }

    /// The face kept at `index` (the database's): face 0 when the data no
    /// longer lists it.
    pub fn kept(self, index: u32) -> Face {
        self.face(index).unwrap_or(Face::DEFAULT)
    }

    /// A server bot's face: one of all the faces, fixed for its match by
    /// the match's `salt` and the bot's `number` in it, so bots of a match
    /// look apart and the same bot looks otherwise in another match.
    pub fn bot_face(self, salt: u64, number: usize) -> Face {
        // splitmix64's finaliser: a well spread number from the two.
        let mut mixed = salt ^ (number as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^= mixed >> 31;
        Face((mixed % u64::from(self.count.get())) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(heads: Option<ConfigValue>) -> Vec<u8> {
        let mut body = vec![("bind_pose".to_owned(), ConfigValue::String("x".into()))];
        if let Some(heads) = heads {
            body.push(("heads".to_owned(), heads));
        }
        ConfigValue::Named(vec![("body_parts".to_owned(), ConfigValue::Named(body))])
            .to_bytes()
            .unwrap()
    }

    fn head(model: &str) -> ConfigValue {
        ConfigValue::Named(vec![(
            "data".to_owned(),
            ConfigValue::Named(vec![
                ("base_model".to_owned(), ConfigValue::String(model.into())),
                ("part_name".to_owned(), ConfigValue::String("p".into())),
            ]),
        )])
    }

    #[test]
    fn the_test_data_has_todays_head_alone() {
        let data = super::super::game_data::shared();
        let root = ConfigValue::parse(data.file(PATH).expect("the bodyparts")).unwrap();
        assert!(root.req("body_parts").unwrap().get("head").is_some());
        let faces = Faces::of_data(data).unwrap();
        assert_eq!(faces.count(), 1);
        assert_eq!(faces.face(0), Some(Face::DEFAULT));
        assert_eq!(faces.face(1), None);
    }

    #[test]
    fn heads_are_the_faces() {
        let faces = Faces::parse(&file(Some(ConfigValue::Indexed(vec![
            head("a"),
            head("b"),
            head("c"),
        ]))))
        .unwrap();
        assert_eq!(faces.count(), 3);
        assert_eq!(faces.face(2).map(Face::index), Some(2));
        assert_eq!(faces.face(3), None);
        assert_eq!(faces.face(256 + 1), None);
        assert_eq!(faces.kept(7), Face::DEFAULT);
        assert_eq!(faces.kept(1).index(), 1);
        assert_eq!(Faces::parse(&file(None)).unwrap().count(), 1);
    }

    #[test]
    fn malformed_heads_are_refused() {
        assert!(Faces::parse(&file(Some(ConfigValue::Integer(3)))).is_err());
        assert!(Faces::parse(&file(Some(ConfigValue::Indexed(Vec::new())))).is_err());
        let no_part = ConfigValue::Named(vec![(
            "data".to_owned(),
            ConfigValue::Named(vec![("base_model".to_owned(), ConfigValue::String("a".into()))]),
        )]);
        assert!(Faces::parse(&file(Some(ConfigValue::Indexed(vec![no_part])))).is_err());
        assert!(Faces::of_count(usize::from(MAX_FACES)).is_ok());
        assert!(Faces::of_count(usize::from(MAX_FACES) + 1).is_err());
    }

    #[test]
    fn bots_get_every_face_and_keep_theirs() {
        let faces = Faces::of_count(10).unwrap();
        let drawn: std::collections::BTreeSet<u8> =
            (0..200).map(|n| faces.bot_face(12345, n).index()).collect();
        assert_eq!(drawn.len(), 10);
        assert_eq!(faces.bot_face(7, 3), faces.bot_face(7, 3));
        assert_eq!(Faces::one().bot_face(7, 3), Face::DEFAULT);
    }
}
