//! A nickname change (`49 03` check, `49 04` change;
//! `docs/notes/lobby_requests.md` §2.1, §4.1).
//!
//! The lobby's rename window (`MessageEnterName`) checks a name as it is
//! typed (`49 03`, answered `83 49 03 <name> <ok> [reason]`) and sends
//! `49 04 <name> <u32>`: the u32 is the item id of the rename voucher the
//! player pays with, or 0 to pay in gold (`change_player_nickname(name,
//! radioButtonPremium.selected ? 0 : voucherId)`; PROVEN). The gold price
//! is 0x8D's fourth value (`root.set_change_name_cost`). The answer is
//! `83 49 04 <voucher item id or 0>`: the client takes one voucher off its
//! copy. The new name itself reaches the client in `8F`.
//!
//! What a name is in srp: an account signs in with its **login** (the
//! user name it registered, `crate::Account::login`) and is shown by its
//! **name** (`crate::Account::name`, copied to the 0.26g account), which
//! starts as the login's part before an `@`. A rename changes the name
//! only: the sign-in is not touched. [choice] A new name must be shown by
//! no other account and be no other account's login (so nobody can pose as
//! the owner of a login); the check for that is the lobby's, which has the
//! store. The price is `SRP_RENAME_COST` gold, by default the rename
//! voucher's own price in the dictionary (250).
//!
//! **Characters** (`docs/notes/names.md`). The movies put a name into the
//! HTML of other players' chat unescaped (`LobbyChat.as::get_styled_text`,
//! `GameChat.as`), so a chosen name takes [`name_char`] only, a new
//! account's name is made of them ([`shown_name`]), and a name saved
//! before has what no name may show made `_` on loading ([`made_safe`]).
//! Either way the name is then one no other account has ([`unique`]).

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::{cut, limits};

use super::account::{Account, cap_name};
use super::premium::{RENAME_VOUCHER, Settings};

/// The letters and digits of every name, an account's or a clan's: the
/// game's own regexes' (`gameplay/clans/clan_name.options`: Latin, digits,
/// Cyrillic `А`..`я`, the Ukrainian and German letters) and `Ё`, `ё`, `Ґ`,
/// `ґ`, which they lack. [choice: for those four] Each kind of name adds
/// its own punctuation ([`name_char`], `clans::name_char`).
pub fn base_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || ('А'..='я').contains(&c) || "ЁёҐґЄєІіЇїÄäÖöÜüß".contains(c)
}

/// A character of a chosen name: [`base_char`] and the `-`, `_`, `+` of
/// the game's `account_name_regex`. Not its blank inside a name (a rename
/// refuses blanks) nor its `|` (its author's separator). No `@` (the
/// owner's rule).
pub fn name_char(c: char) -> bool {
    base_char(c) || "-_+".contains(c)
}

/// What no name may show, however old: markup (`<`, `>`, `&`, `"`), the
/// end of the name's link (`'`, `href='event:name:<name>: '`), its field
/// separator (`:`, which `LobbyChat.as::clickLinkHandler` splits on, so
/// the player menu would act on another name), `@` (the owner's rule),
/// control characters, and the invisible ones that turn or hide the text
/// around them ([`format_char`]).
fn unsafe_char(c: char) -> bool {
    c.is_control() || format_char(c) || "<>&\"':@".contains(c)
}

/// Unicode's format characters (category Cf: the direction overrides and
/// isolates, zero-width spaces and joiners, the byte-order mark, the tags)
/// and the line and paragraph separators: nothing a reader sees, so a name
/// with one can pose as another or turn the line it is in.
fn format_char(c: char) -> bool {
    matches!(
        u32::from(c),
        0xAD | 0x600..=0x605
            | 0x61C
            | 0x6DD
            | 0x70F
            | 0x890..=0x891
            | 0x8E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Whether two names are one for the players: the same but for case.
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// The name an account gets: `name` cut to the client's 63 bytes, or
/// `fallback` when it has no letter or digit (a login of only marks or
/// letters no name takes is all `_`), and then, while `taken` says another
/// account is shown by it, with `_2`, `_3`, ... (cut so the number fits).
pub fn unique(name: &str, fallback: &str, taken: impl Fn(&str) -> bool) -> String {
    let name = if name.chars().any(char::is_alphanumeric) {
        name
    } else {
        fallback
    };
    let name = cap_name(name);
    if !taken(&name) {
        return name;
    }
    (2u32..)
        .map(|n| {
            let suffix = format!("_{n}");
            format!("{}{suffix}", cut(&name, limits::NAME - suffix.len()))
        })
        .find(|candidate| !taken(candidate))
        .expect("a number no account has")
}

/// The name a new account is shown by, from its login's part before an
/// `@`: each character a chosen name may not have is made `_`.
pub fn shown_name(login_part: &str) -> String {
    login_part
        .chars()
        .map(|c| if name_char(c) { c } else { '_' })
        .collect()
}

/// A saved name with what no name may show ([`unsafe_char`]) made `_`, or
/// `None` when it has none. Only those: a name merely outside
/// [`name_char`] (a `.` of a login) stays as players know it.
pub fn made_safe(name: &str) -> Option<String> {
    name.chars().any(unsafe_char).then(|| {
        name.chars()
            .map(|c| if unsafe_char(c) { '_' } else { c })
            .collect()
    })
}

/// Why a rename is refused once the name itself is good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameError {
    /// The item named is no rename voucher of the account.
    NoVoucher,
    NotEnoughGold,
}

impl RenameError {
    /// The game's own text for the refusal.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NoVoucher => "st_change_name_error",
            Self::NotEnoughGold => "st_cant_buy_premium_message",
        }
    }
}

/// How a rename was paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paid {
    /// The voucher one was taken of; 0 when gold paid.
    pub voucher_item_id: u32,
    pub gold: u32,
}

/// `49 04`: pay for the rename and take the name (cut to what the client's
/// buffers hold). `voucher_item_id` 0 pays in gold. The caller has checked
/// the name.
pub fn rename(
    account: &mut Account,
    settings: &Settings,
    name: &str,
    voucher_item_id: u32,
) -> Result<Paid, RenameError> {
    let paid = if voucher_item_id == 0 {
        if account.premium_money < settings.rename_cost {
            return Err(RenameError::NotEnoughGold);
        }
        account.premium_money -= settings.rename_cost;
        Paid {
            voucher_item_id: 0,
            gold: settings.rename_cost,
        }
    } else {
        let voucher = Dictionary::shared().item_named(RENAME_VOUCHER);
        let index = account
            .inventory
            .iter()
            .position(|item| {
                item.item_id == voucher_item_id
                    && voucher.is_some_and(|voucher| voucher.dict_id == item.dict_id)
                    && item.condition_or_stack > 0
            })
            .ok_or(RenameError::NoVoucher)?;
        let item = &mut account.inventory[index];
        item.condition_or_stack -= 1;
        if item.condition_or_stack == 0 {
            account.inventory.remove(index);
            account.sync_kits();
        }
        Paid {
            voucher_item_id,
            gold: 0,
        }
    };
    account.name = cap_name(name);
    Ok(paid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_is_paid_in_gold_or_with_a_voucher() {
        let settings = Settings::default();
        let mut account = Account::new("Alice");
        account.premium_money = 300;
        assert_eq!(
            rename(&mut account, &settings, "Alisa", 0),
            Ok(Paid {
                voucher_item_id: 0,
                gold: 250
            })
        );
        assert_eq!(
            (account.name.as_str(), account.premium_money),
            ("Alisa", 50)
        );
        // Not the gold for a second one.
        assert_eq!(
            rename(&mut account, &settings, "Alice", 0),
            Err(RenameError::NotEnoughGold)
        );
        assert_eq!(account.name, "Alisa");
        // A voucher pays instead, one of the stack.
        let dictionary = Dictionary::shared();
        let voucher = account.add_item(dictionary.item_named(RENAME_VOUCHER).unwrap().dict_id, 2);
        let paid = rename(&mut account, &settings, "Alice", voucher).unwrap();
        assert_eq!((paid.voucher_item_id, paid.gold), (voucher, 0));
        assert_eq!(account.item(voucher).unwrap().condition_or_stack, 1);
        assert_eq!(
            (account.name.as_str(), account.premium_money),
            ("Alice", 50)
        );
        rename(&mut account, &settings, "Alina", voucher).unwrap();
        assert!(account.item(voucher).is_none());
        // Gone, or not a rename voucher: refused, the name stays.
        let pistol = account.kits[0].slots[7].unwrap().item_id;
        for item in [voucher, pistol, 9_999] {
            assert_eq!(
                rename(&mut account, &settings, "Other", item),
                Err(RenameError::NoVoucher)
            );
        }
        assert_eq!(account.name, "Alina");
        // The name is cut to the client's buffer.
        account.premium_money = 250;
        rename(&mut account, &settings, &"x".repeat(100), 0).unwrap();
        assert_eq!(account.name.len(), 63);
        // A price of 0 is free.
        let free = Settings {
            rename_cost: 0,
            ..settings
        };
        assert_eq!(rename(&mut account, &free, "Alice", 0).unwrap().gold, 0);
    }

    #[test]
    fn a_name_takes_the_games_characters_and_no_at() {
        for good in ["Stalker_1", "Сталкер", "Їжак-2", "Ёжик", "Ґава", "Müller+"]
        {
            assert!(good.chars().all(name_char), "{good}");
        }
        for bad in [
            '@', '<', '>', '&', '\'', '"', ':', '.', ' ', '|', '[', ']', '\u{7}',
        ] {
            assert!(!name_char(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_new_accounts_name_is_made_of_name_characters() {
        assert_eq!(shown_name("john.doe"), "john_doe");
        assert_eq!(shown_name("<b>x</b>"), "_b_x__b_");
        assert_eq!(shown_name("Сталкер"), "Сталкер");
    }

    #[test]
    fn a_saved_name_loses_only_what_no_name_may_show() {
        assert_eq!(made_safe("john.doe"), None);
        assert_eq!(made_safe("Сталкер 1"), None);
        assert_eq!(
            made_safe("<font size='99'>a</font>").as_deref(),
            Some("_font size=_99__a_/font_")
        );
        assert_eq!(made_safe("evil:victim").as_deref(), Some("evil_victim"));
        assert_eq!(made_safe("a@b&c\"\u{1}").as_deref(), Some("a_b_c__"));
    }

    #[test]
    fn invisible_and_turning_characters_are_in_no_name() {
        // Right-to-left override, zero-width space and joiner, an isolate,
        // the byte-order mark, a tag, the line separator.
        let hidden = [
            '\u{202E}',
            '\u{200B}',
            '\u{200D}',
            '\u{2066}',
            '\u{2069}',
            '\u{FEFF}',
            '\u{E0041}',
            '\u{2028}',
            '\u{AD}',
        ];
        for c in hidden {
            assert!(!name_char(c), "{c:?}");
            assert!(!shown_name(&format!("a{c}b")).contains(c), "{c:?}");
            assert_eq!(
                made_safe(&format!("a{c}b")).as_deref(),
                Some("a_b"),
                "{c:?}"
            );
        }
    }

    #[test]
    fn a_name_is_made_unique_and_has_a_letter() {
        let taken = ["bob", "bob_2"];
        let taken = |name: &str| taken.iter().any(|other| same_name(other, name));
        assert_eq!(unique("alice", "player_1", taken), "alice");
        assert_eq!(unique("BOB", "player_1", taken), "BOB_3");
        assert_eq!(unique("___", "player_1", taken), "player_1");
        assert_eq!(unique("", "player_1", taken), "player_1");
        // The number fits the client's 63 bytes, at a character's end.
        let long = "я".repeat(40);
        let taken_long = |name: &str| name == cap_name(&long);
        let made = unique(&long, "player_1", taken_long);
        assert!(made.len() <= 63 && made.ends_with("_2"), "{made}");
    }

    #[test]
    fn clans_and_accounts_share_their_letters() {
        for c in ['Ё', 'ё', 'Ґ', 'ґ', 'Ї', 'ß', 'Z', '7'] {
            assert!(base_char(c) && name_char(c), "{c:?}");
        }
        assert!(!base_char('_') && name_char('_'));
    }

    #[test]
    fn the_refusals_have_the_games_texts() {
        assert_eq!(RenameError::NoVoucher.text_key(), "st_change_name_error");
        assert_eq!(
            RenameError::NotEnoughGold.text_key(),
            "st_cant_buy_premium_message"
        );
    }
}
