//! The wiki's icons, from the game's lobby (`lobby_menu.swf`): each item's,
//! each modification's (`ui_desc.icon`) and each item group's (the shop's
//! filter buttons). Built per release with the calculator
//! (`srp-026g/scripts/damage-calc/build_site.py`, through
//! `srp-026g/scripts/wiki/build_icons.py`) into `../wiki/icons/`; every file
//! is part of the binary, `../wiki/icons/files.rs` lists them. The
//! modification and group icons are white glyphs on transparent, for the
//! page to tint (CSS masks).

use std::collections::HashMap;
use std::sync::OnceLock;

/// The icon sets, each a folder of `../wiki/icons/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconSet {
    /// By the item's `items_dict` name.
    Item,
    /// By the modification's `ui_desc.icon`.
    Modifier,
    /// By `items_dict`'s `item_category`.
    Group,
}

impl IconSet {
    fn folder(self) -> &'static str {
        match self {
            Self::Item => "items",
            Self::Modifier => "mods",
            Self::Group => "groups",
        }
    }
}

/// Where the files are served.
const PREFIX: &str = "/wiki/icons/";

macro_rules! files {
    ($($path:literal => $content_type:literal,)*) => {
        &[$((
            $path,
            $content_type,
            // Relative to files.rs, which is included.
            include_bytes!(concat!("../icons/", $path)) as &[u8],
        ),)*]
    };
}

/// The icons: path under [`PREFIX`], content type, bytes.
const FILES: &[(&str, &str, &[u8])] = include!("../wiki/icons/files.rs");

/// Each icon's address with its version, by its path under [`PREFIX`].
fn urls() -> &'static HashMap<&'static str, String> {
    static URLS: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    URLS.get_or_init(|| {
        FILES
            .iter()
            .map(|&(path, _, bytes)| {
                let version: String = openssl::sha::sha256(bytes)[..6]
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                (path, format!("{PREFIX}{path}?v={version}"))
            })
            .collect()
    })
}

/// The address of `key`'s icon in `set`, with its version (`?v=`), if the
/// build made one.
pub(crate) fn url(set: IconSet, key: &str) -> Option<String> {
    urls()
        .get(format!("{}/{key}.webp", set.folder()).as_str())
        .cloned()
}

/// The icon file at `path` (the target before any `?`): its content type
/// and bytes.
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    let path = path.strip_prefix(PREFIX)?;
    FILES
        .iter()
        .find(|&&(at, _, _)| at == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::dictionary::Dictionary;
    use survarium::v026g::item_params;
    use survarium::v026g::modifications::{self, Modifications};

    /// The categories the wiki groups items by: the shop's equipment and
    /// weapon filters.
    const GROUPS: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 13, 14, 15, 16, 17];

    #[test]
    fn every_item_that_takes_a_modification_has_an_icon() {
        let dictionary = Dictionary::shared();
        let mut missing = Vec::new();
        for item in &dictionary.items {
            let rolls =
                modifications::item_mods(item.dict_id).is_some_and(|mods| mods.modifications != 0);
            let built_in = item_params::item_params(item.dict_id)
                .is_some_and(|params| !params.default_modifications.is_empty());
            if (rolls || built_in)
                && GROUPS.contains(&item.category)
                && url(IconSet::Item, &item.name).is_none()
            {
                missing.push(item.name.as_str());
            }
        }
        assert!(missing.is_empty(), "no icon: {missing:?}");
    }

    #[test]
    fn every_modification_and_group_has_an_icon() {
        for modification in &Modifications::shared().modifications {
            let icon = modification.shown.icon;
            assert!(
                url(IconSet::Modifier, &icon.to_string()).is_some(),
                "modification {} shows icon {icon}",
                modification.id
            );
        }
        for category in GROUPS {
            assert!(
                url(IconSet::Group, &category.to_string()).is_some(),
                "category {category}"
            );
        }
    }

    #[test]
    fn an_address_serves_its_file() {
        let address = url(IconSet::Group, "1").unwrap();
        let (path, version) = address.split_once("?v=").unwrap();
        assert_eq!(path, "/wiki/icons/groups/1.webp");
        assert_eq!(version.len(), 12);
        let (content_type, bytes) = file(path).unwrap();
        assert_eq!(content_type, "image/webp");
        assert!(bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP");
        assert!(url(IconSet::Item, "no_such_item").is_none());
        assert!(file("/wiki/icons/groups/0.webp").is_none());
        assert!(file("/wiki/icons/../icons.rs").is_none());
    }
}
