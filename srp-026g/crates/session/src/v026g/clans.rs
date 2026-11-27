//! Clans and clan teams (`docs/notes/social.md` §3).
//!
//! What the client shows of a clan (PROVEN in its lobby SWF and the
//! handlers of 0x9A, see the notes):
//! - A clan starts as a **registration**: a player reserves a name and an
//!   abbreviation (`49 06`), invites players (`49 0F`), silver is paid into
//!   the registration (`49 0C`, by the founder or a player who accepted),
//!   and the founder registers the clan (`49 07`) once it is paid and
//!   enough players accepted (the count of 0x9C's first byte). Whoever
//!   leaves a registration loses what it paid (`st_leave_clan_money_message`).
//! - A registered clan has up to 10 ranks, the first the commander's; a
//!   rank's five permissions are bits of one byte ([`permission`]). Members
//!   are ranked up and down one rank at a time; ranking a member up to the
//!   commander's rank hands the clan over
//!   (`st_rank_up_to_commander_message`).
//! - A clan has one **clan team** at a time: a field commander (role 0),
//!   the lineup (1), the reserve (2) and applications (3). The lineup and
//!   the commander play: soldiers press Play to be ready, then the
//!   commander does, and the team waits for another clan's team.
//!
//! What the client does not show is srp's own, kept simple: who may do
//! what to whom (a member acts only on members of a lower rank), where a
//! new rank goes (above the lowest), a clan's experience (its members'
//! match experience) and levels. The numbers an operator may want to
//! change are [`Config`], read from the environment.
//!
//! [`Clans`] is the store: every rule is a method that changes it or
//! refuses with the game's own text key ([`ClanError`]). Clans are kept in
//! `srp.db` (`g26_clans*`, `persist`); invites and clan teams are not (a
//! clan team is a lobby thing, like a squad).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};

use crate::LockExt;

/// The permissions of a rank: the bits of the byte 0x9A carries (PROVEN:
/// `set_clan_manage_permissions` 0x2A11A0 unpacks them in this order).
pub mod permission {
    pub const BATTLE_COMMANDER: u8 = 0x01;
    pub const DAILY_MESSAGE: u8 = 0x02;
    pub const CHANGE_RANK: u8 = 0x04;
    pub const KICK: u8 = 0x08;
    pub const INVITE: u8 = 0x10;
    pub const ALL: u8 = 0x1F;

    /// The bit of a permission as `49 10` names it: 0 invite, 1 kick,
    /// 2 change rank, 3 daily message, 4 battle commander (the constants of
    /// the five `update_*_permission` callbacks, 0x2A2570..0x2A2670).
    pub fn from_code(code: u8) -> Option<u8> {
        Some(match code {
            0 => INVITE,
            1 => KICK,
            2 => CHANGE_RANK,
            3 => DAILY_MESSAGE,
            4 => BATTLE_COMMANDER,
            _ => return None,
        })
    }
}

/// A clan-team member's role (`TournamentWindow.STATE_*`).
pub mod role {
    pub const COMMANDER: u8 = 0;
    pub const SOLDIER: u8 = 1;
    pub const RESERVE: u8 = 2;
    /// "Applications": asked to join, not taken yet.
    pub const APPLICANT: u8 = 3;
}

/// A player's state in a registration (`ClanInviteRenderer.statusMap`).
pub mod status {
    pub const ACCEPTED: u8 = 0;
    pub const REJECTED: u8 = 1;
    pub const PENDING: u8 = 2;
}

/// The most ranks a clan has (the client's array, and `set_clan_ranks`
/// offers a new one only under 10).
pub const MAX_RANKS: usize = 10;
/// The most members the client's list holds.
pub const MAX_MEMBERS: usize = 105;
/// The most clan-team members the client's list holds.
pub const MAX_TEAM_MEMBERS: usize = 100;
/// A rank's name: the client's 32-byte buffer.
pub const MAX_RANK_NAME: usize = 31;
/// The daily message, in UTF-16 units (the client's buffer).
pub const MAX_DAILY_MESSAGE: usize = 245;
/// An abbreviation is 3 to 6 characters (`st_clan_abbreviation_info`).
pub const TAG_CHARS: std::ops::RangeInclusive<usize> = 3..=6;
/// [choice] A name is 3 to 24 characters (48 bytes of Cyrillic: inside the
/// client's 63).
pub const NAME_CHARS: std::ops::RangeInclusive<usize> = 3..=24;
/// The default ranks, by the keys the client names them with
/// (`st_clan_rank_<name>`), highest first, and what each may do [choice].
const DEFAULT_RANKS: [(&str, u8); 4] = [
    ("commander", permission::ALL),
    ("assistant", permission::ALL),
    ("warlord", permission::INVITE | permission::BATTLE_COMMANDER),
    ("soldier", 0),
];

/// The numbers of the clan rules the client does not fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Silver to register a clan (0x8D's `clan_creation`).
    pub cost: u32,
    /// Players who must have accepted, the founder among them, before a
    /// clan is registered (0x9C's first byte).
    pub players_required: u8,
    /// The least and most players (the commander and the lineup) a clan
    /// team presses Play with (0x9C's second and third byte).
    pub team_min: u8,
    pub team_max: u8,
    /// The most members of a clan.
    pub max_members: u8,
}

impl Default for Config {
    /// [choice] srp's defaults. A clan costs 50,000 silver and has at
    /// most 30 members: the owner's choice of 2026-10-12 (the only source
    /// is the Russian wiki of mid 2015, 0.29 to 0.30: 250,000 silver and
    /// 15 members; nothing says what 0.26 had). A founder registers alone
    /// and a clan team is one player, so nothing waits for more players
    /// than a test has; the game's text speaks of 5 players
    /// (`st_clan_creation_info_description`), which the live server sets
    /// (`SRP_CLAN_PLAYERS_REQUIRED=5`).
    fn default() -> Self {
        Self {
            cost: 50_000,
            players_required: 1,
            team_min: 1,
            team_max: 10,
            max_members: 30,
        }
    }
}

impl Config {
    /// `SRP_CLAN_COST`, `SRP_CLAN_PLAYERS_REQUIRED`, `SRP_CLAN_TEAM_MIN`,
    /// `SRP_CLAN_TEAM_MAX`, `SRP_CLAN_MAX_MEMBERS`; a bad value is logged
    /// and the default kept.
    pub fn from_env() -> Self {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let default = Self::default();
        let number = |name: &str, default: u32, range: std::ops::RangeInclusive<u32>| {
            let Some(value) = var(name) else {
                return default;
            };
            match value.trim().parse::<u32>() {
                Ok(number) if range.contains(&number) => number,
                _ => {
                    log::error!(
                        "{name} is `{value}`; it must be a number of {}..={}. Using {default}",
                        range.start(),
                        range.end()
                    );
                    default
                }
            }
        };
        let mut config = Self {
            cost: number("SRP_CLAN_COST", default.cost, 0..=100_000_000),
            players_required: number(
                "SRP_CLAN_PLAYERS_REQUIRED",
                default.players_required.into(),
                1..=100,
            ) as u8,
            team_min: number("SRP_CLAN_TEAM_MIN", default.team_min.into(), 1..=10) as u8,
            team_max: number("SRP_CLAN_TEAM_MAX", default.team_max.into(), 1..=10) as u8,
            max_members: number(
                "SRP_CLAN_MAX_MEMBERS",
                default.max_members.into(),
                1..=MAX_MEMBERS as u32,
            ) as u8,
        };
        if config.team_min > config.team_max {
            log::error!("SRP_CLAN_TEAM_MIN is over SRP_CLAN_TEAM_MAX; using the maximum for both");
            config.team_min = config.team_max;
        }
        config
    }

    /// The environment's, read once.
    pub fn shared() -> &'static Config {
        static SHARED: OnceLock<Config> = OnceLock::new();
        SHARED.get_or_init(Self::from_env)
    }
}

/// `9C`: the rules' numbers as the client is told them at sign-in.
pub fn limits(config: &Config) -> survarium::v026g::lobby::ServerMessage {
    survarium::v026g::lobby::clan_limits(config.players_required, config.team_min, config.team_max)
}

/// Why a clan request is refused: the game's own text key for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanError(pub &'static str);

impl ClanError {
    pub fn text_key(self) -> &'static str {
        self.0
    }
}

type Res<T> = Result<T, ClanError>;

/// What is wrong with a clan name or abbreviation, in the client's five
/// reasons (`83 49 08` / `83 49 09`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameFault {
    /// `st_short_clan_string`.
    Short = 0,
    /// `st_few_alpha_clan_string` ("Incorrect name"): too few letters, or
    /// too long (the client has no text of its own for that).
    FewLetters = 1,
    /// `st_already_exist_clan_string`.
    Taken = 2,
    /// `st_space_symbol_clan_string` ("Invalid delimiters").
    Delimiters = 3,
    /// `st_wrong_symbol_clan_string`.
    WrongSymbol = 4,
}

/// The characters of `clan_name_regex` (the game's
/// `gameplay/clans/clan_name.options`): Cyrillic, Latin, digits, the
/// Ukrainian and German letters, `@`, `#`, the blank and `-`. (The `|` the
/// regex's class also holds is its author's separator, not taken.)
fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || ('А'..='я').contains(&c) || "єЄІіЇїÄäÖöÜüß@# -".contains(c)
}

/// The Latin letter a look-alike stands for (`clan_name_letters_replacement`
/// of the same file): names that read the same are the same name.
fn look_alike(c: char) -> char {
    match c {
        'а' | 'А' | 'ä' | 'Ä' => 'A',
        'в' | 'В' => 'B',
        'с' | 'С' => 'C',
        'е' | 'Е' | 'ё' | 'Ё' => 'E',
        'н' | 'Н' => 'H',
        'і' | 'І' | 'ї' | 'Ї' => 'I',
        'к' | 'К' => 'K',
        'м' | 'М' => 'M',
        'о' | 'О' | 'ö' | 'Ö' => 'O',
        'р' | 'Р' => 'P',
        'ß' => 'S',
        'т' | 'Т' => 'T',
        'ü' | 'Ü' => 'U',
        'х' | 'Х' => 'X',
        'у' | 'У' => 'Y',
        other => other,
    }
}

/// A name as it is compared: look-alikes as their Latin letter, no case.
pub fn folded(name: &str) -> String {
    name.chars()
        .map(look_alike)
        .flat_map(char::to_lowercase)
        .collect()
}

/// What is wrong with the form of a clan name (`tag` false) or an
/// abbreviation (true). [choice, but for the characters and the
/// abbreviation's length, which are the game's.]
pub fn name_fault(text: &str, tag: bool) -> Option<NameFault> {
    let allowed = if tag { TAG_CHARS } else { NAME_CHARS };
    let count = text.chars().count();
    if count < *allowed.start() {
        return Some(NameFault::Short);
    }
    if !text.chars().all(name_char) {
        return Some(NameFault::WrongSymbol);
    }
    let blanks = if tag {
        text.contains(' ')
    } else {
        text.starts_with(' ') || text.ends_with(' ') || text.contains("  ")
    };
    if blanks {
        return Some(NameFault::Delimiters);
    }
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if count > *allowed.end() || letters < if tag { 1 } else { 2 } {
        return Some(NameFault::FewLetters);
    }
    None
}

/// A rank of a clan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rank {
    /// A default rank's name is the key the client names it with.
    pub name: String,
    pub permissions: u8,
    pub is_default: bool,
}

/// A member of a clan, or a player invited to its registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    pub account_id: u32,
    /// The index of its rank; 0 is the commander's.
    pub rank: u8,
    /// [`status`]; always accepted in a registered clan.
    pub status: u8,
}

/// A clan, registered or being registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clan {
    pub id: u32,
    pub name: String,
    pub tag: String,
    pub registered: bool,
    /// The founder of a registration; the commander of a clan.
    pub leader: u32,
    pub ranks: Vec<Rank>,
    /// In the order they joined (were invited).
    pub members: Vec<Member>,
    pub daily_message: String,
    pub experience: u32,
    /// The silver paid into the registration.
    pub money: u32,
}

/// The experience a clan has at the start of `level` [choice]: 10 000 for
/// the second level, 20 000 more for the third, and so on.
pub fn level_start(level: u8) -> u32 {
    let level = u32::from(level.max(1));
    5_000_u32.saturating_mul(level * (level - 1))
}

/// The highest level [choice].
pub const MAX_LEVEL: u8 = 50;

/// The level of a clan with `experience`.
pub fn level_of(experience: u32) -> u8 {
    let mut level = 1;
    while level < MAX_LEVEL && experience >= level_start(level + 1) {
        level += 1;
    }
    level
}

impl Clan {
    pub fn member(&self, account_id: u32) -> Option<&Member> {
        self.members
            .iter()
            .find(|member| member.account_id == account_id)
    }

    fn member_mut(&mut self, account_id: u32) -> Option<&mut Member> {
        self.members
            .iter_mut()
            .find(|member| member.account_id == account_id)
    }

    /// The members that count: of a registration, who accepted.
    pub fn accepted(&self) -> impl Iterator<Item = &Member> {
        self.members
            .iter()
            .filter(|member| member.status == status::ACCEPTED)
    }

    /// Whether the account may do what the permission names.
    pub fn may(&self, account_id: u32, permission: u8) -> bool {
        account_id == self.leader
            || self
                .member(account_id)
                .filter(|member| member.status == status::ACCEPTED)
                .and_then(|member| self.ranks.get(usize::from(member.rank)))
                .is_some_and(|rank| rank.permissions & permission != 0)
    }

    /// The level, and the experience it starts and ends at.
    pub fn level(&self) -> (u8, u32, u32) {
        let level = level_of(self.experience);
        (level, level_start(level), level_start(level + 1))
    }

    /// Whether the registration can be finished: paid, with enough players.
    pub fn can_register(&self, config: &Config) -> bool {
        !self.registered
            && self.money >= config.cost
            && self.accepted().count() >= usize::from(config.players_required)
    }
}

/// A member of a clan team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamMember {
    pub account_id: u32,
    /// [`role`].
    pub role: u8,
    pub ready: bool,
    /// The kit of its last clan Play (`30`).
    pub kit_id: u32,
}

/// A clan's team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub id: u32,
    pub clan_id: u32,
    /// The commander first, then by role (the client's window needs them
    /// grouped), each group in the order they came.
    pub members: Vec<TeamMember>,
}

impl Team {
    pub fn member(&self, account_id: u32) -> Option<&TeamMember> {
        self.members
            .iter()
            .find(|member| member.account_id == account_id)
    }

    fn member_mut(&mut self, account_id: u32) -> Option<&mut TeamMember> {
        self.members
            .iter_mut()
            .find(|member| member.account_id == account_id)
    }

    pub fn commander(&self) -> Option<&TeamMember> {
        self.members
            .iter()
            .find(|member| member.role == role::COMMANDER)
    }

    /// Who plays: the commander and the lineup.
    pub fn battle(&self) -> impl Iterator<Item = &TeamMember> {
        self.members
            .iter()
            .filter(|member| member.role <= role::SOLDIER)
    }

    /// Who is of the team (and its chat): all but the applicants.
    pub fn taken(&self) -> impl Iterator<Item = &TeamMember> {
        self.members
            .iter()
            .filter(|member| member.role <= role::RESERVE)
    }

    /// The team waits for a match once its commander is ready.
    pub fn is_queued(&self) -> bool {
        self.commander().is_some_and(|commander| commander.ready)
    }

    fn sort(&mut self) {
        self.members.sort_by_key(|member| member.role);
    }

    /// Anything changed: the team does not wait for a match any more.
    fn unqueue(&mut self) {
        for member in &mut self.members {
            if member.role == role::COMMANDER {
                member.ready = false;
            }
        }
    }
}

/// An invite waiting for its answer (`49 0A`, `49 1A`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    pub clan_id: u32,
    pub inviter: u32,
    pub target: u32,
    /// To the clan team, not the clan.
    pub team: bool,
    /// The id of the important message that carries it.
    pub message_id: u32,
    pub text: String,
}

/// How a player left its clan (`49 0D`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Departure {
    /// It was the commander (or the founder): the clan is no more. The
    /// clan as it was.
    Disbanded(Clan),
    /// It left: the clan as it is now.
    Left(Clan),
}

/// The answer to a rank-up request (`49 12`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RankUp {
    /// The member would become the commander: the client asks its player
    /// and sends `49 11 <id> 1`.
    Confirm,
    Done(Clan),
}

struct Inner {
    config: Config,
    clans: BTreeMap<u32, Clan>,
    /// The clan of each account that is of one (accepted, or the founder).
    by_member: HashMap<u32, u32>,
    /// The clan teams, by clan.
    teams: HashMap<u32, Team>,
    invites: Vec<Invite>,
    next_id: u32,
    next_team_id: u32,
    /// What the repository does not have yet.
    dirty: BTreeSet<u32>,
    deleted: Vec<u32>,
}

/// The most invites kept waiting; past it the oldest goes.
const MAX_INVITES: usize = 10_000;

mod text {
    pub const RESERVE: &str = "st_cant_reserve_clan";
    pub const CREATE: &str = "st_cant_create_clan";
    pub const PAY: &str = "st_cant_buy_clan_registration";
    pub const LEAVE: &str = "st_cant_cancel_clan_registration";
    pub const INVITE: &str = "st_cant_invite_player_to_clan";
    pub const IN_CLAN: &str = "st_player_is_already_in_clan";
    pub const FULL: &str = "st_clan_max_members_count_error";
    pub const ADD: &str = "st_cant_add_clan_member";
    pub const PERMISSION: &str = "st_cant_change_permission";
    pub const RANK: &str = "st_cant_change_rank_clan_member";
    pub const RANK_UP: &str = "st_cant_rank_up_clan_member";
    pub const KICK: &str = "st_cant_kick_member";
    pub const KICK_SELF: &str = "st_cant_kick_himself";
    pub const CREATE_RANK: &str = "st_cant_create_clan_rank";
    pub const REMOVE_RANK: &str = "st_cant_remove_clan_rank";
    pub const DAILY: &str = "st_cant_set_daily_message";
    pub const TEAM_CREATE: &str = "st_cant_create_clan_team";
    pub const TEAM_HAS_COMMANDER: &str = "st_team_already_has_commander";
    pub const TEAM_JOIN: &str = "st_cant_request_to_join_clan_team";
    pub const TEAM_IN: &str = "st_already_in_clan_team";
    pub const TEAM_HAS_STATUS: &str = "st_already_has_status_clan_team";
    pub const TEAM_MAX: &str = "st_ct_max_members_count_error";
    pub const TEAM_FULL: &str = "st_clan_team_is_full";
    pub const TEAM_INVITE: &str = "st_cant_invite_player_to_clan_team";
    pub const TEAM_ADD: &str = "st_cant_add_clan_team_member";
    pub const TEAM_LEAVE: &str = "st_cant_leave_clan_team";
    pub const TEAM_DISBAND: &str = "st_cant_disband_clan_team";
    pub const TEAM_STATUS: &str = "st_cant_change_status_clan_team_member";
    pub const TEAM_OUST: &str = "st_cant_oust_commander";
    pub const TEAM_REMOVE: &str = "st_cant_remove_clan_team_member";
    pub const TEAM_READY: &str = "st_cant_change_ready_state";
    pub const TEAM_COMMANDER: &str = "st_cant_set_clan_team_commander";
    pub const TEAM_PLAY: &str = "st_must_be_in_clan_team";
}

/// Every text key a clan request can be refused with, for a check against
/// the game's localization.
pub const TEXT_KEYS: [&str; 33] = [
    text::RESERVE,
    text::CREATE,
    text::PAY,
    text::LEAVE,
    text::INVITE,
    text::IN_CLAN,
    text::FULL,
    text::ADD,
    text::PERMISSION,
    text::RANK,
    text::RANK_UP,
    text::KICK,
    text::KICK_SELF,
    text::CREATE_RANK,
    text::REMOVE_RANK,
    text::DAILY,
    text::TEAM_CREATE,
    text::TEAM_HAS_COMMANDER,
    text::TEAM_JOIN,
    text::TEAM_IN,
    text::TEAM_HAS_STATUS,
    text::TEAM_MAX,
    text::TEAM_FULL,
    text::TEAM_INVITE,
    text::TEAM_ADD,
    text::TEAM_LEAVE,
    text::TEAM_DISBAND,
    text::TEAM_STATUS,
    text::TEAM_OUST,
    text::TEAM_REMOVE,
    text::TEAM_READY,
    text::TEAM_COMMANDER,
    text::TEAM_PLAY,
];

/// The clans of a running srp.
pub struct Clans {
    inner: Mutex<Inner>,
}

impl Default for Clans {
    fn default() -> Self {
        Self::new()
    }
}

impl Clans {
    /// No clans, the environment's rules.
    pub fn new() -> Self {
        Self::loaded(Vec::new())
    }

    /// The clans a repository kept.
    pub fn loaded(clans: Vec<Clan>) -> Self {
        let mut by_member = HashMap::new();
        for clan in &clans {
            by_member.insert(clan.leader, clan.id);
            for member in clan.accepted() {
                by_member.insert(member.account_id, clan.id);
            }
        }
        Self {
            inner: Mutex::new(Inner {
                config: *Config::shared(),
                next_id: clans.iter().map(|clan| clan.id).max().unwrap_or(0) + 1,
                clans: clans.into_iter().map(|clan| (clan.id, clan)).collect(),
                by_member,
                teams: HashMap::new(),
                invites: Vec::new(),
                next_team_id: 1,
                dirty: BTreeSet::new(),
                deleted: Vec::new(),
            }),
        }
    }

    pub fn config(&self) -> Config {
        self.inner.locked().config
    }

    /// Other rules than the environment's (tests).
    pub fn set_config(&self, config: Config) {
        self.inner.locked().config = config;
    }

    // ------------------------------------------------------------ lookups

    pub fn clan(&self, clan_id: u32) -> Option<Clan> {
        self.inner.locked().clans.get(&clan_id).cloned()
    }

    /// The clan (or registration) the account is of.
    pub fn clan_of(&self, account_id: u32) -> Option<Clan> {
        let inner = self.inner.locked();
        inner.clan_of(account_id).cloned()
    }

    /// The abbreviation an account shows: its registered clan's.
    pub fn tag_of(&self, account_id: u32) -> Option<String> {
        let inner = self.inner.locked();
        inner
            .clan_of(account_id)
            .filter(|clan| clan.registered)
            .map(|clan| clan.tag.clone())
    }

    pub fn team(&self, clan_id: u32) -> Option<Team> {
        self.inner.locked().teams.get(&clan_id).cloned()
    }

    /// The team the account is of (applicants are of none).
    pub fn team_of(&self, account_id: u32) -> Option<Team> {
        let inner = self.inner.locked();
        let clan_id = inner.by_member.get(&account_id)?;
        inner
            .teams
            .get(clan_id)
            .filter(|team| team.taken().any(|member| member.account_id == account_id))
            .cloned()
    }

    pub fn len(&self) -> usize {
        self.inner.locked().clans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// What is wrong with a name someone asks about: its form, or a clan
    /// has it (the asker's own registration aside).
    pub fn check_name(&self, name: &str, asker: u32) -> Option<NameFault> {
        name_fault(name, false).or_else(|| {
            let inner = self.inner.locked();
            inner.name_taken(name, asker).then_some(NameFault::Taken)
        })
    }

    pub fn check_tag(&self, tag: &str, asker: u32) -> Option<NameFault> {
        name_fault(tag, true).or_else(|| {
            let inner = self.inner.locked();
            inner.tag_taken(tag, asker).then_some(NameFault::Taken)
        })
    }

    // -------------------------------------------------------- registration

    /// `49 06`: reserve a name and an abbreviation. The asker becomes the
    /// founder of a registration; one it already founded is renamed.
    pub fn reserve(&self, founder: u32, name: &str, tag: &str) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::RESERVE);
        if name_fault(name, false).is_some()
            || name_fault(tag, true).is_some()
            || inner.name_taken(name, founder)
            || inner.tag_taken(tag, founder)
        {
            return Err(refused);
        }
        if let Some(clan_id) = inner.by_member.get(&founder).copied() {
            let clan = inner.clans.get_mut(&clan_id).ok_or(refused)?;
            if clan.registered || clan.leader != founder {
                return Err(ClanError(text::IN_CLAN));
            }
            clan.name = name.to_owned();
            clan.tag = tag.to_owned();
            let clan = clan.clone();
            inner.dirty.insert(clan_id);
            return Ok(clan);
        }
        let id = inner.next_id;
        inner.next_id += 1;
        let clan = Clan {
            id,
            name: name.to_owned(),
            tag: tag.to_owned(),
            registered: false,
            leader: founder,
            ranks: DEFAULT_RANKS
                .iter()
                .map(|&(name, permissions)| Rank {
                    name: name.to_owned(),
                    permissions,
                    is_default: true,
                })
                .collect(),
            members: vec![Member {
                account_id: founder,
                rank: 0,
                status: status::ACCEPTED,
            }],
            daily_message: String::new(),
            experience: 0,
            money: 0,
        };
        inner.clans.insert(id, clan.clone());
        inner.by_member.insert(founder, id);
        inner.dirty.insert(id);
        Ok(clan)
    }

    /// How much of `amount` a registration still takes from the account
    /// (`49 0C`); nothing is changed. The founder and whoever accepted may
    /// pay.
    pub fn payable(&self, account_id: u32, amount: u32) -> Res<u32> {
        let inner = self.inner.locked();
        let clan = inner
            .clan_of(account_id)
            .filter(|clan| !clan.registered)
            .ok_or(ClanError(text::PAY))?;
        let due = inner.config.cost.saturating_sub(clan.money);
        if amount == 0 || due == 0 {
            return Err(ClanError(text::PAY));
        }
        Ok(amount.min(due))
    }

    /// Pay `amount` (what [`Clans::payable`] gave) into the registration.
    pub fn pay(&self, account_id: u32, amount: u32) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let clan = inner
            .clan_of_mut(account_id)
            .filter(|clan| !clan.registered)
            .ok_or(ClanError(text::PAY))?;
        clan.money = clan.money.saturating_add(amount);
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Ok(clan)
    }

    /// `49 07`: the founder registers the clan. Players who did not accept
    /// are no part of it: they are returned with the clan.
    pub fn create(&self, founder: u32) -> Res<(Clan, Vec<u32>)> {
        let mut inner = self.inner.locked();
        let config = inner.config;
        let clan = inner
            .clan_of_mut(founder)
            .filter(|clan| clan.leader == founder && clan.can_register(&config))
            .ok_or(ClanError(text::CREATE))?;
        let lowest = clan.ranks.len().saturating_sub(1) as u8;
        let dropped = clan
            .members
            .iter()
            .filter(|member| member.status != status::ACCEPTED)
            .map(|member| member.account_id)
            .collect();
        clan.members
            .retain(|member| member.status == status::ACCEPTED);
        for member in &mut clan.members {
            member.rank = if member.account_id == founder {
                0
            } else {
                lowest
            };
        }
        clan.registered = true;
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        inner.invites.retain(|invite| invite.clan_id != clan.id);
        Ok((clan, dropped))
    }

    /// `49 0D`: leave the clan (or the registration); its commander (or
    /// founder) disbands it.
    pub fn leave_or_disband(&self, account_id: u32) -> Res<Departure> {
        let mut inner = self.inner.locked();
        let clan_id = *inner
            .by_member
            .get(&account_id)
            .ok_or(ClanError(text::LEAVE))?;
        let leader = inner
            .clans
            .get(&clan_id)
            .ok_or(ClanError(text::LEAVE))?
            .leader;
        if leader == account_id {
            let clan = inner.clans.remove(&clan_id).ok_or(ClanError(text::LEAVE))?;
            inner.by_member.retain(|_, clan| *clan != clan_id);
            inner.teams.remove(&clan_id);
            inner.invites.retain(|invite| invite.clan_id != clan_id);
            inner.dirty.remove(&clan_id);
            inner.deleted.push(clan_id);
            return Ok(Departure::Disbanded(clan));
        }
        Ok(Departure::Left(inner.remove_member(clan_id, account_id)))
    }

    // ------------------------------------------------------------- invites

    /// Whether `inviter` may invite `target` to its clan (`49 0F`): the
    /// clan, if so.
    pub fn may_invite(&self, inviter: u32, target: u32) -> Res<Clan> {
        let inner = self.inner.locked();
        inner.may_invite(inviter, target).cloned()
    }

    /// The invite was sent (the long `4D` of type 7): it waits for the
    /// answer under `message_id`. A registration lists the player as
    /// pending.
    pub fn invite(&self, inviter: u32, target: u32, message_id: u32, text: &str) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let clan_id = inner.may_invite(inviter, target)?.id;
        inner.keep_invite(Invite {
            clan_id,
            inviter,
            target,
            team: false,
            message_id,
            text: text.to_owned(),
        });
        let clan = inner
            .clans
            .get_mut(&clan_id)
            .ok_or(ClanError(text::INVITE))?;
        if !clan.registered {
            let lowest = clan.ranks.len().saturating_sub(1) as u8;
            match clan.member_mut(target) {
                Some(member) => member.status = status::PENDING,
                None => clan.members.push(Member {
                    account_id: target,
                    rank: lowest,
                    status: status::PENDING,
                }),
            }
        }
        let clan = clan.clone();
        inner.dirty.insert(clan_id);
        Ok(clan)
    }

    /// The invites waiting for a player (it signed in).
    pub fn invites_for(&self, target: u32) -> Vec<Invite> {
        let inner = self.inner.locked();
        inner
            .invites
            .iter()
            .filter(|invite| invite.target == target)
            .cloned()
            .collect()
    }

    /// `49 0A <inviter> <answer>`: the clan as it is after the answer, and
    /// whether the player is now of it.
    pub fn answer_invite(&self, target: u32, inviter: u32, accept: bool) -> Res<(Clan, bool)> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::ADD);
        let position = inner
            .invites
            .iter()
            .position(|invite| !invite.team && invite.target == target && invite.inviter == inviter)
            .ok_or(refused)?;
        let invite = inner.invites.remove(position);
        let clan_id = invite.clan_id;
        let max_members = usize::from(inner.config.max_members);
        if accept && inner.by_member.contains_key(&target) {
            return Err(ClanError(text::IN_CLAN));
        }
        let clan = inner.clans.get_mut(&clan_id).ok_or(refused)?;
        let lowest = clan.ranks.len().saturating_sub(1) as u8;
        if clan.registered {
            if !accept {
                return Ok((clan.clone(), false));
            }
            if clan.members.len() >= max_members {
                return Err(ClanError(text::FULL));
            }
            clan.members.push(Member {
                account_id: target,
                rank: lowest,
                status: status::ACCEPTED,
            });
        } else {
            if accept && clan.accepted().count() >= max_members {
                return Err(ClanError(text::FULL));
            }
            let member = clan.member_mut(target).ok_or(refused)?;
            member.status = if accept {
                status::ACCEPTED
            } else {
                status::REJECTED
            };
        }
        let clan = clan.clone();
        inner.dirty.insert(clan_id);
        if accept {
            inner.by_member.insert(target, clan_id);
            // Its other invites are void.
            inner
                .invites
                .retain(|invite| invite.team || invite.target != target);
        }
        Ok((clan, accept))
    }

    // ------------------------------------------------- members and ranks

    /// `49 13`: kick a member of a lower rank.
    pub fn kick(&self, actor: u32, target: u32) -> Res<Clan> {
        let mut inner = self.inner.locked();
        if actor == target {
            return Err(ClanError(text::KICK_SELF));
        }
        let clan = inner.acts_on(actor, target, permission::KICK, text::KICK)?;
        let clan_id = clan.id;
        Ok(inner.remove_member(clan_id, target))
    }

    /// `49 12`: rank a member up one rank. Up to the commander's rank needs
    /// the commander's word first.
    pub fn ask_rank_up(&self, actor: u32, target: u32) -> Res<RankUp> {
        let mut inner = self.inner.locked();
        let clan = inner.acts_on(actor, target, permission::CHANGE_RANK, text::RANK_UP)?;
        let (actor_rank, target_rank) = (
            clan.member(actor).map_or(0, |member| member.rank),
            clan.member(target).map_or(0, |member| member.rank),
        );
        if target_rank == 1 && clan.leader == actor {
            return Ok(RankUp::Confirm);
        }
        // Never up to the actor's own rank.
        if target_rank <= actor_rank + 1 {
            return Err(ClanError(text::RANK_UP));
        }
        let clan_id = clan.id;
        Ok(RankUp::Done(inner.set_rank(
            clan_id,
            target,
            target_rank - 1,
        )))
    }

    /// `49 11 <id> <up>`: down one rank; or up, which the client sends only
    /// as the commander's yes to handing the clan over.
    pub fn change_rank(&self, actor: u32, target: u32, up: bool) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let clan = inner.acts_on(actor, target, permission::CHANGE_RANK, text::RANK)?;
        let clan_id = clan.id;
        let target_rank = clan.member(target).map_or(0, |member| member.rank);
        let lowest = clan.ranks.len().saturating_sub(1) as u8;
        if !up {
            if target_rank >= lowest {
                return Err(ClanError(text::RANK));
            }
            return Ok(inner.set_rank(clan_id, target, target_rank + 1));
        }
        if !(target_rank == 1 && clan.leader == actor) {
            drop(inner);
            return match self.ask_rank_up(actor, target) {
                Ok(RankUp::Done(clan)) => Ok(clan),
                _ => Err(ClanError(text::RANK)),
            };
        }
        // The clan changes hands: the two swap ranks.
        inner.set_rank(clan_id, actor, 1);
        let clan = inner.clans.get_mut(&clan_id).ok_or(ClanError(text::RANK))?;
        clan.leader = target;
        Ok(inner.set_rank(clan_id, target, 0))
    }

    /// `49 10 <rank> <value> <permission>`: the commander sets what a rank
    /// below its own may do.
    pub fn set_permission(&self, actor: u32, rank: u8, code: u8, value: bool) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::PERMISSION);
        let bit = permission::from_code(code).ok_or(refused)?;
        let clan = inner.led_by(actor).ok_or(refused)?;
        if rank == 0 {
            return Err(refused);
        }
        let rank = clan.ranks.get_mut(usize::from(rank)).ok_or(refused)?;
        if value {
            rank.permissions |= bit;
        } else {
            rank.permissions &= !bit;
        }
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Ok(clan)
    }

    /// `49 14`: a new rank, just above the lowest (new members get the
    /// lowest), with no permissions.
    pub fn create_rank(&self, actor: u32, name: &str) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::CREATE_RANK);
        let clan = inner.led_by(actor).ok_or(refused)?;
        let name = name.trim();
        if clan.ranks.len() >= MAX_RANKS
            || name.is_empty()
            || name.len() > MAX_RANK_NAME
            || name.chars().any(char::is_control)
            || clan
                .ranks
                .iter()
                .any(|rank| rank.name.eq_ignore_ascii_case(name))
        {
            return Err(refused);
        }
        let lowest = clan.ranks.len() - 1;
        clan.ranks.insert(
            lowest,
            Rank {
                name: name.to_owned(),
                permissions: 0,
                is_default: false,
            },
        );
        for member in &mut clan.members {
            if usize::from(member.rank) >= lowest {
                member.rank += 1;
            }
        }
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Ok(clan)
    }

    /// `49 15`: remove a rank that is no default one; its members get the
    /// next lower.
    pub fn remove_rank(&self, actor: u32, rank: u8) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::REMOVE_RANK);
        let clan = inner.led_by(actor).ok_or(refused)?;
        let index = usize::from(rank);
        if clan.ranks.get(index).is_none_or(|rank| rank.is_default) {
            return Err(refused);
        }
        clan.ranks.remove(index);
        for member in &mut clan.members {
            if usize::from(member.rank) > index {
                member.rank -= 1;
            }
        }
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Ok(clan)
    }

    /// `49 16`: the daily message, cut to what the client's buffer holds.
    pub fn set_daily_message(&self, actor: u32, message: &str) -> Res<Clan> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::DAILY);
        let clan = inner
            .clan_of_mut(actor)
            .filter(|clan| clan.registered && clan.may(actor, permission::DAILY_MESSAGE))
            .ok_or(refused)?;
        let mut units = 0;
        clan.daily_message = message
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take_while(|c| {
                units += c.len_utf16();
                units <= MAX_DAILY_MESSAGE
            })
            .collect();
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Ok(clan)
    }

    /// A member earned experience in a match: its registered clan gets it.
    /// The clan, and the level it reached if it did.
    pub fn add_experience(&self, account_id: u32, experience: u32) -> Option<(Clan, Option<u8>)> {
        let mut inner = self.inner.locked();
        let clan = inner
            .clan_of_mut(account_id)
            .filter(|clan| clan.registered && experience > 0)?;
        let before = clan.level().0;
        clan.experience = clan.experience.saturating_add(experience);
        let after = clan.level().0;
        let clan = clan.clone();
        inner.dirty.insert(clan.id);
        Some((clan, (after > before).then_some(after)))
    }

    // ---------------------------------------------------------- clan teams

    /// `49 17`: make the clan's team, or take the command of one that has
    /// no commander. For a member who may command in battle.
    pub fn create_team(&self, actor: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_CREATE);
        let clan = inner
            .clan_of(actor)
            .filter(|clan| clan.registered && clan.may(actor, permission::BATTLE_COMMANDER))
            .ok_or(refused)?;
        let clan_id = clan.id;
        if inner
            .teams
            .get(&clan_id)
            .is_some_and(|team| team.commander().is_some())
        {
            return Err(ClanError(text::TEAM_HAS_COMMANDER));
        }
        let id = inner.next_team_id;
        inner.next_team_id = inner.next_team_id.wrapping_add(1).max(1);
        let team = inner.teams.entry(clan_id).or_insert(Team {
            id,
            clan_id,
            members: Vec::new(),
        });
        team.members.retain(|member| member.account_id != actor);
        team.members.insert(
            0,
            TeamMember {
                account_id: actor,
                role: role::COMMANDER,
                ready: false,
                kit_id: 0,
            },
        );
        Ok(team.clone())
    }

    /// `49 18`: ask to join the clan's team: listed under "applications".
    pub fn ask_to_join_team(&self, actor: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let clan_id = inner
            .clan_of(actor)
            .filter(|clan| clan.registered)
            .ok_or(ClanError(text::TEAM_JOIN))?
            .id;
        let team = inner
            .teams
            .get_mut(&clan_id)
            .ok_or(ClanError(text::TEAM_JOIN))?;
        if team.member(actor).is_some() {
            return Err(ClanError(text::TEAM_IN));
        }
        if team.members.len() >= MAX_TEAM_MEMBERS {
            return Err(ClanError(text::TEAM_MAX));
        }
        team.members.push(TeamMember {
            account_id: actor,
            role: role::APPLICANT,
            ready: false,
            kit_id: 0,
        });
        Ok(team.clone())
    }

    /// Whether the team's commander may invite `target` (`49 19`).
    pub fn may_invite_to_team(&self, actor: u32, target: u32) -> Res<Team> {
        let inner = self.inner.locked();
        inner.may_invite_to_team(actor, target).cloned()
    }

    /// The team invite was sent (the long `4D` of type 10).
    pub fn invite_to_team(
        &self,
        actor: u32,
        target: u32,
        message_id: u32,
        text: &str,
    ) -> Res<Team> {
        let mut inner = self.inner.locked();
        let team = inner.may_invite_to_team(actor, target)?.clone();
        inner.keep_invite(Invite {
            clan_id: team.clan_id,
            inviter: actor,
            target,
            team: true,
            message_id,
            text: text.to_owned(),
        });
        Ok(team)
    }

    /// `49 1A <inviter> <answer>`: join the lineup, or the reserve when the
    /// lineup is full. `None`: declined.
    pub fn answer_team_invite(&self, target: u32, inviter: u32, accept: bool) -> Res<Option<Team>> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_ADD);
        let position = inner
            .invites
            .iter()
            .position(|invite| invite.team && invite.target == target && invite.inviter == inviter)
            .ok_or(refused)?;
        let invite = inner.invites.remove(position);
        if !accept {
            return Ok(None);
        }
        let team_max = usize::from(inner.config.team_max);
        if inner.by_member.get(&target) != Some(&invite.clan_id) {
            return Err(refused);
        }
        let team = inner.teams.get_mut(&invite.clan_id).ok_or(refused)?;
        if team.taken().any(|member| member.account_id == target) {
            return Err(ClanError(text::TEAM_IN));
        }
        if team.members.len() >= MAX_TEAM_MEMBERS {
            return Err(ClanError(text::TEAM_MAX));
        }
        team.members.retain(|member| member.account_id != target);
        let role = if team.battle().count() < team_max {
            role::SOLDIER
        } else {
            role::RESERVE
        };
        team.members.push(TeamMember {
            account_id: target,
            role,
            ready: false,
            kit_id: 0,
        });
        team.unqueue();
        team.sort();
        Ok(Some(team.clone()))
    }

    /// `49 1B`, `49 23`: leave the team (an applicant takes its application
    /// back). A team its commander leaves has none until someone takes the
    /// command; an empty one is no more. The clan, and the team if one is
    /// left.
    pub fn leave_team(&self, actor: u32) -> Res<(u32, Option<Team>)> {
        let mut inner = self.inner.locked();
        let clan_id = *inner
            .by_member
            .get(&actor)
            .ok_or(ClanError(text::TEAM_LEAVE))?;
        if inner
            .teams
            .get(&clan_id)
            .is_none_or(|team| team.member(actor).is_none())
        {
            return Err(ClanError(text::TEAM_LEAVE));
        }
        Ok((clan_id, inner.drop_from_team(clan_id, actor)))
    }

    /// `49 1C`: the team's commander, or the clan's, disbands the team. The
    /// team as it was.
    pub fn disband_team(&self, actor: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_DISBAND);
        let clan = inner.clan_of(actor).ok_or(refused)?;
        let (clan_id, leads) = (clan.id, clan.leader == actor);
        let commands = inner
            .teams
            .get(&clan_id)
            .and_then(Team::commander)
            .is_some_and(|commander| commander.account_id == actor);
        if !leads && !commands {
            return Err(refused);
        }
        let team = inner.teams.remove(&clan_id).ok_or(refused)?;
        inner
            .invites
            .retain(|invite| !(invite.team && invite.clan_id == clan_id));
        Ok(team)
    }

    /// `49 1D <name> <status>`: the commander moves a member to the lineup
    /// (1) or the reserve (2).
    pub fn set_team_status(&self, actor: u32, target: u32, status: u8) -> Res<Team> {
        let mut inner = self.inner.locked();
        let team_max = usize::from(inner.config.team_max);
        let team = inner.commanded_by(actor, text::TEAM_STATUS)?;
        if actor == target || !matches!(status, role::SOLDIER | role::RESERVE) {
            return Err(ClanError(text::TEAM_STATUS));
        }
        let battle = team.battle().count();
        let member = team
            .member_mut(target)
            .ok_or(ClanError(text::TEAM_STATUS))?;
        if status == role::SOLDIER && member.role != role::SOLDIER && battle >= team_max {
            return Err(ClanError(text::TEAM_FULL));
        }
        member.role = status;
        member.ready = false;
        team.unqueue();
        team.sort();
        Ok(team.clone())
    }

    /// `49 1E`: the commander steps down to the lineup, by its own word or
    /// the clan commander's.
    pub fn oust_commander(&self, actor: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_OUST);
        let clan = inner.clan_of(actor).ok_or(refused)?;
        let (clan_id, leads) = (clan.id, clan.leader == actor);
        let team = inner.teams.get_mut(&clan_id).ok_or(refused)?;
        let commander = team
            .members
            .iter_mut()
            .find(|member| member.role == role::COMMANDER)
            .ok_or(refused)?;
        if !leads && commander.account_id != actor {
            return Err(refused);
        }
        commander.role = role::SOLDIER;
        commander.ready = false;
        team.sort();
        Ok(team.clone())
    }

    /// `49 1F <name>`: the commander removes a member (or turns an
    /// application down).
    pub fn remove_team_member(&self, actor: u32, target: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let team = inner.commanded_by(actor, text::TEAM_REMOVE)?;
        if actor == target || team.member(target).is_none() {
            return Err(ClanError(text::TEAM_REMOVE));
        }
        team.members.retain(|member| member.account_id != target);
        team.unqueue();
        Ok(team.clone())
    }

    /// `49 22 <name>`: the clan's commander names the team's commander; the
    /// old one joins the lineup.
    pub fn set_team_commander(&self, actor: u32, target: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_COMMANDER);
        let clan = inner
            .clan_of(actor)
            .filter(|clan| clan.leader == actor)
            .ok_or(refused)?;
        let clan_id = clan.id;
        let team = inner.teams.get_mut(&clan_id).ok_or(refused)?;
        if team.member(target).is_none() {
            return Err(refused);
        }
        for member in &mut team.members {
            if member.account_id == target {
                member.role = role::COMMANDER;
                member.ready = false;
            } else if member.role == role::COMMANDER {
                member.role = role::SOLDIER;
                member.ready = false;
            }
        }
        team.sort();
        Ok(team.clone())
    }

    /// `30 <kit>`: clan Play, for the commander and the lineup. The kit is
    /// kept for the match.
    pub fn clan_play(&self, actor: u32, kit_id: u32) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_PLAY);
        let clan_id = *inner.by_member.get(&actor).ok_or(refused)?;
        let team = inner.teams.get_mut(&clan_id).ok_or(refused)?;
        let member = team
            .member_mut(actor)
            .filter(|member| member.role <= role::SOLDIER)
            .ok_or(refused)?;
        member.kit_id = kit_id;
        Ok(team.clone())
    }

    /// `49 20 <ready>`: a soldier of the lineup is ready, or not any more.
    /// Not while the commander is ready (the client's Play is off then).
    pub fn soldier_ready(&self, actor: u32, ready: bool) -> Res<Team> {
        let mut inner = self.inner.locked();
        let refused = ClanError(text::TEAM_READY);
        let clan_id = *inner.by_member.get(&actor).ok_or(refused)?;
        let team = inner.teams.get_mut(&clan_id).ok_or(refused)?;
        if team.is_queued() {
            return Err(refused);
        }
        let member = team
            .member_mut(actor)
            .filter(|member| member.role == role::SOLDIER)
            .ok_or(refused)?;
        member.ready = ready;
        Ok(team.clone())
    }

    /// `49 21 <ready>`: the commander is ready (the team waits for a match:
    /// every soldier of the lineup must be ready and the lineup of the
    /// size 0x9C allows, as the client's Play button has it), or not any
    /// more.
    pub fn commander_ready(&self, actor: u32, ready: bool) -> Res<Team> {
        let mut inner = self.inner.locked();
        let config = inner.config;
        let team = inner.commanded_by(actor, text::TEAM_READY)?;
        if ready {
            let battle = team.battle().count();
            let all_ready = team
                .battle()
                .all(|member| member.ready || member.role == role::COMMANDER);
            if !all_ready
                || battle < usize::from(config.team_min)
                || battle > usize::from(config.team_max)
            {
                return Err(ClanError(text::TEAM_READY));
            }
        }
        for member in &mut team.members {
            if member.role == role::COMMANDER {
                member.ready = ready;
            }
        }
        Ok(team.clone())
    }

    /// The clan's team does not wait for a match any more (its match was
    /// made, or could not be): nobody is ready. The team, if it changed.
    pub fn team_unready(&self, clan_id: u32) -> Option<Team> {
        let mut inner = self.inner.locked();
        let team = inner.teams.get_mut(&clan_id)?;
        if !team.members.iter().any(|member| member.ready) {
            return None;
        }
        for member in &mut team.members {
            member.ready = false;
        }
        Some(team.clone())
    }

    /// The player is not ready any more (it left the queue, or the lobby):
    /// its team, if that changed it. A team it was ready in does not wait.
    pub fn member_unready(&self, account_id: u32) -> Option<Team> {
        let mut inner = self.inner.locked();
        let clan_id = *inner.by_member.get(&account_id)?;
        let team = inner.teams.get_mut(&clan_id)?;
        let member = team.member_mut(account_id)?;
        let was_ready = std::mem::take(&mut member.ready);
        let in_battle = member.role <= role::SOLDIER;
        let queued = team.is_queued();
        if !was_ready && !(in_battle && queued) {
            return None;
        }
        team.unqueue();
        Some(team.clone())
    }

    // ---------------------------------------------------------- repository

    /// What changed since the last call: the clans to write and the ids to
    /// delete.
    pub fn take_changes(&self) -> (Vec<Clan>, Vec<u32>) {
        let mut inner = self.inner.locked();
        let dirty = std::mem::take(&mut inner.dirty);
        let clans = dirty
            .iter()
            .filter_map(|id| inner.clans.get(id).cloned())
            .collect();
        (clans, std::mem::take(&mut inner.deleted))
    }

    /// A write failed: it is tried again on the next pass.
    pub fn mark_unsaved(&self, clan_id: u32, deleted: bool) {
        let mut inner = self.inner.locked();
        if deleted {
            inner.deleted.push(clan_id);
        } else if inner.clans.contains_key(&clan_id) {
            inner.dirty.insert(clan_id);
        }
    }
}

impl Inner {
    fn clan_of(&self, account_id: u32) -> Option<&Clan> {
        self.clans.get(self.by_member.get(&account_id)?)
    }

    fn clan_of_mut(&mut self, account_id: u32) -> Option<&mut Clan> {
        self.clans.get_mut(self.by_member.get(&account_id)?)
    }

    /// The registered clan the account commands.
    fn led_by(&mut self, account_id: u32) -> Option<&mut Clan> {
        self.clan_of_mut(account_id)
            .filter(|clan| clan.registered && clan.leader == account_id)
    }

    fn name_taken(&self, name: &str, asker: u32) -> bool {
        let name = folded(name);
        self.clans
            .values()
            .any(|clan| !(clan.leader == asker && !clan.registered) && folded(&clan.name) == name)
    }

    fn tag_taken(&self, tag: &str, asker: u32) -> bool {
        let tag = folded(tag);
        self.clans
            .values()
            .any(|clan| !(clan.leader == asker && !clan.registered) && folded(&clan.tag) == tag)
    }

    fn keep_invite(&mut self, invite: Invite) {
        self.invites.retain(|kept| {
            (kept.clan_id, kept.target, kept.team) != (invite.clan_id, invite.target, invite.team)
        });
        if self.invites.len() >= MAX_INVITES {
            self.invites.remove(0);
        }
        self.invites.push(invite);
    }

    fn may_invite(&self, inviter: u32, target: u32) -> Res<&Clan> {
        let refused = ClanError(text::INVITE);
        let clan = self.clan_of(inviter).ok_or(refused)?;
        // A registration is its founder's to fill.
        let may = if clan.registered {
            clan.may(inviter, permission::INVITE)
        } else {
            clan.leader == inviter
        };
        if !may || inviter == target {
            return Err(refused);
        }
        if self.by_member.contains_key(&target) {
            return Err(ClanError(text::IN_CLAN));
        }
        if clan.accepted().count() >= usize::from(self.config.max_members)
            || clan.members.len() >= MAX_MEMBERS
        {
            return Err(ClanError(text::FULL));
        }
        Ok(clan)
    }

    /// The registered clan in which `actor`, with the permission, acts on
    /// `target`, a member of a lower rank.
    fn acts_on(
        &self,
        actor: u32,
        target: u32,
        permission: u8,
        refusal: &'static str,
    ) -> Res<&Clan> {
        let refused = ClanError(refusal);
        let clan = self
            .clan_of(actor)
            .filter(|clan| clan.registered && clan.may(actor, permission))
            .ok_or(refused)?;
        let actor_rank = clan.member(actor).ok_or(refused)?.rank;
        let target_rank = clan.member(target).ok_or(refused)?.rank;
        if actor == target || target_rank <= actor_rank {
            return Err(refused);
        }
        Ok(clan)
    }

    fn set_rank(&mut self, clan_id: u32, account_id: u32, rank: u8) -> Clan {
        let clan = self.clans.get_mut(&clan_id).expect("the caller found it");
        if let Some(member) = clan.member_mut(account_id) {
            member.rank = rank;
        }
        self.dirty.insert(clan_id);
        clan.clone()
    }

    /// Take an account out of a clan it does not command, and out of the
    /// clan's team.
    fn remove_member(&mut self, clan_id: u32, account_id: u32) -> Clan {
        self.by_member.remove(&account_id);
        self.drop_from_team(clan_id, account_id);
        self.invites
            .retain(|invite| !(invite.clan_id == clan_id && invite.inviter == account_id));
        self.dirty.insert(clan_id);
        let clan = self.clans.get_mut(&clan_id).expect("the caller found it");
        clan.members
            .retain(|member| member.account_id != account_id);
        clan.clone()
    }

    /// The team, if one is left.
    fn drop_from_team(&mut self, clan_id: u32, account_id: u32) -> Option<Team> {
        let team = self.teams.get_mut(&clan_id)?;
        if team.member(account_id).is_some() {
            team.unqueue();
            team.members
                .retain(|member| member.account_id != account_id);
        }
        if team.members.is_empty() {
            self.teams.remove(&clan_id);
            self.invites
                .retain(|invite| !(invite.team && invite.clan_id == clan_id));
            return None;
        }
        Some(team.clone())
    }

    fn may_invite_to_team(&self, actor: u32, target: u32) -> Res<&Team> {
        let refused = ClanError(text::TEAM_INVITE);
        let clan_id = self.by_member.get(&actor).ok_or(refused)?;
        let team = self.teams.get(clan_id).ok_or(refused)?;
        if team.commander().map(|commander| commander.account_id) != Some(actor)
            || actor == target
            || self.by_member.get(&target) != Some(clan_id)
        {
            return Err(refused);
        }
        if team.taken().any(|member| member.account_id == target) {
            return Err(ClanError(text::TEAM_HAS_STATUS));
        }
        if team.members.len() >= MAX_TEAM_MEMBERS {
            return Err(ClanError(text::TEAM_MAX));
        }
        Ok(team)
    }

    /// The team `actor` commands.
    fn commanded_by(&mut self, actor: u32, refusal: &'static str) -> Res<&mut Team> {
        let refused = ClanError(refusal);
        let clan_id = *self.by_member.get(&actor).ok_or(refused)?;
        self.teams
            .get_mut(&clan_id)
            .filter(|team| team.commander().map(|commander| commander.account_id) == Some(actor))
            .ok_or(refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clan rules these tests were written with, said outright: a clan
    /// for 1,000 silver (a new account's 2,000 pays it) and 100 members.
    /// srp's defaults are other numbers ([`Config::default`]).
    fn test_config() -> Config {
        Config {
            cost: 1_000,
            players_required: 1,
            team_min: 1,
            team_max: 10,
            max_members: 100,
        }
    }

    fn store(config: Config) -> Clans {
        let clans = Clans::new();
        clans.set_config(config);
        clans
    }

    /// A registered clan of accounts 1 (the commander), 2 and 3.
    fn clan_of_three() -> Clans {
        let clans = store(Config {
            players_required: 3,
            ..test_config()
        });
        clans.reserve(1, "Stalkers", "STK").unwrap();
        for (target, message) in [(2, 10), (3, 11)] {
            clans.invite(1, target, message, "Stalkers").unwrap();
            assert!(clans.answer_invite(target, 1, true).unwrap().1);
        }
        clans.pay(1, 1_000).unwrap();
        clans.create(1).unwrap();
        clans
    }

    #[test]
    fn the_settings_come_from_the_environment_with_defaults() {
        assert_eq!(Config::from_vars(|_| None), Config::default());
        // srp's defaults: 50,000 silver and 30 members (the owner's choice),
        // registered alone.
        assert_eq!(
            Config::default(),
            Config {
                cost: 50_000,
                players_required: 1,
                team_min: 1,
                team_max: 10,
                max_members: 30,
            }
        );
        let config = Config::from_vars(|name| {
            Some(
                match name {
                    "SRP_CLAN_COST" => "500000",
                    "SRP_CLAN_PLAYERS_REQUIRED" => " 5 ",
                    "SRP_CLAN_TEAM_MIN" => "4",
                    "SRP_CLAN_TEAM_MAX" => "8",
                    _ => "50",
                }
                .to_owned(),
            )
        });
        assert_eq!(
            config,
            Config {
                cost: 500_000,
                players_required: 5,
                team_min: 4,
                team_max: 8,
                max_members: 50
            }
        );
        // Nonsense keeps the default; a minimum over the maximum is cut.
        let config = Config::from_vars(|name| {
            Some(
                match name {
                    "SRP_CLAN_TEAM_MIN" => "9",
                    "SRP_CLAN_TEAM_MAX" => "3",
                    "SRP_CLAN_MAX_MEMBERS" => "106",
                    _ => "many",
                }
                .to_owned(),
            )
        });
        assert_eq!(
            config,
            Config {
                team_min: 3,
                team_max: 3,
                ..Config::default()
            }
        );
    }

    #[test]
    fn names_are_checked_by_the_games_own_characters() {
        assert_eq!(name_fault("ab", false), Some(NameFault::Short));
        assert_eq!(name_fault("AB", true), Some(NameFault::Short));
        assert_eq!(name_fault("Clan!", false), Some(NameFault::WrongSymbol));
        assert_eq!(name_fault("Кл|ан", false), Some(NameFault::WrongSymbol));
        assert_eq!(name_fault(" Clan", false), Some(NameFault::Delimiters));
        assert_eq!(name_fault("My  Clan", false), Some(NameFault::Delimiters));
        assert_eq!(name_fault("A B", true), Some(NameFault::Delimiters));
        assert_eq!(name_fault("12345", false), Some(NameFault::FewLetters));
        assert_eq!(name_fault("ABCDEFG", true), Some(NameFault::FewLetters));
        assert_eq!(
            name_fault(&"x".repeat(25), false),
            Some(NameFault::FewLetters)
        );
        for good in ["Stalkers", "Черный рынок", "Fo-o #1"] {
            assert_eq!(name_fault(good, false), None, "{good}");
        }
        for good in ["STK", "ЧР-1", "A1@#bc"] {
            assert_eq!(name_fault(good, true), None, "{good}");
        }
        // A 24-character Cyrillic name fits the client's 63 bytes.
        assert!("я".repeat(*NAME_CHARS.end()).len() <= 63);
        assert!("я".repeat(*TAG_CHARS.end()).len() <= 14);
    }

    #[test]
    fn a_name_that_reads_the_same_is_taken() {
        let clans = store(test_config());
        clans.reserve(1, "BOCTOK", "KOT").unwrap();
        // The same in Cyrillic letters: В, О, С, Т, О, К; К, О, Т.
        assert_eq!(clans.check_name("ВОСТОК", 2), Some(NameFault::Taken));
        assert_eq!(clans.check_name("boctok", 2), Some(NameFault::Taken));
        assert_eq!(clans.check_tag("КОТ", 2), Some(NameFault::Taken));
        assert_eq!(clans.check_name("Boctoks", 2), None);
        // Its founder may ask about its own, and reserve another.
        assert_eq!(clans.check_name("BOCTOK", 1), None);
        assert_eq!(
            clans.reserve(2, "Boctok", "NEW"),
            Err(ClanError(text::RESERVE))
        );
        let renamed = clans.reserve(1, "Vipers", "VPR").unwrap();
        assert_eq!((renamed.id, renamed.name.as_str()), (1, "Vipers"));
        assert_eq!(clans.len(), 1);
        assert_eq!(clans.check_name("BOCTOK", 2), None);
    }

    #[test]
    fn a_registration_is_paid_filled_and_registered() {
        let clans = store(Config {
            players_required: 2,
            cost: 1_500,
            ..test_config()
        });
        let config = clans.config();
        let clan = clans.reserve(1, "Stalkers", "STK").unwrap();
        assert!(!clan.registered);
        assert_eq!(clan.leader, 1);
        assert_eq!(clan.ranks.len(), 4);
        assert_eq!(clan.ranks[0].name, "commander");
        assert!(clan.ranks.iter().all(|rank| rank.is_default));
        // No tag is shown before the clan is registered.
        assert_eq!(clans.tag_of(1), None);
        assert_eq!(clans.create(1), Err(ClanError(text::CREATE)));

        // Paying: never more than is due.
        assert_eq!(clans.payable(1, 1_000), Ok(1_000));
        assert_eq!(clans.pay(1, 1_000).unwrap().money, 1_000);
        assert_eq!(clans.payable(1, 1_000), Ok(500));
        assert_eq!(clans.payable(9, 1_000), Err(ClanError(text::PAY)));

        // An invited player is pending, then accepted or rejected.
        assert_eq!(clans.may_invite(1, 1), Err(ClanError(text::INVITE)));
        let clan = clans.invite(1, 2, 70, "Stalkers").unwrap();
        assert_eq!(clan.members[1].status, status::PENDING);
        clans.invite(1, 3, 71, "Stalkers").unwrap();
        assert_eq!(clans.invites_for(2)[0].message_id, 70);
        assert_eq!(clans.clan_of(2), None);
        // Only who accepted (or founded) may pay.
        assert_eq!(clans.payable(2, 100), Err(ClanError(text::PAY)));
        let (clan, joined) = clans.answer_invite(3, 1, false).unwrap();
        assert!(!joined);
        assert_eq!(clan.members[2].status, status::REJECTED);
        assert!(!clan.can_register(&config));
        let (clan, joined) = clans.answer_invite(2, 1, true).unwrap();
        assert!(joined);
        assert_eq!(clan.accepted().count(), 2);
        // An answer needs its invite.
        assert_eq!(clans.answer_invite(2, 1, true), Err(ClanError(text::ADD)));
        // The accepted player pays the rest.
        assert_eq!(clans.pay(2, 500).unwrap().money, 1_500);
        assert_eq!(clans.payable(2, 1), Err(ClanError(text::PAY)));
        // Only the founder registers.
        assert_eq!(clans.create(2), Err(ClanError(text::CREATE)));
        let (clan, dropped) = clans.create(1).unwrap();
        assert!(clan.registered);
        assert_eq!(dropped, [3]);
        assert_eq!(
            clan.members,
            [
                Member {
                    account_id: 1,
                    rank: 0,
                    status: 0
                },
                Member {
                    account_id: 2,
                    rank: 3,
                    status: 0
                }
            ]
        );
        assert_eq!(clans.tag_of(2).as_deref(), Some("STK"));
        assert_eq!(clans.create(1), Err(ClanError(text::CREATE)));
    }

    #[test]
    fn leaving_a_registration_and_cancelling_it() {
        let clans = store(test_config());
        clans.reserve(1, "Stalkers", "STK").unwrap();
        clans.invite(1, 2, 70, "Stalkers").unwrap();
        clans.answer_invite(2, 1, true).unwrap();
        let Departure::Left(clan) = clans.leave_or_disband(2).unwrap() else {
            panic!("a member leaves");
        };
        assert_eq!(clan.members.len(), 1);
        assert_eq!(clans.clan_of(2), None);
        assert_eq!(clans.leave_or_disband(2), Err(ClanError(text::LEAVE)));
        let Departure::Disbanded(clan) = clans.leave_or_disband(1).unwrap() else {
            panic!("the founder cancels");
        };
        assert_eq!(clan.id, 1);
        assert!(clans.is_empty());
        assert_eq!(clans.take_changes(), (Vec::new(), vec![1]));
        // The name is free again.
        assert_eq!(clans.check_name("Stalkers", 5), None);
    }

    #[test]
    fn a_member_is_invited_to_a_registered_clan() {
        let clans = clan_of_three();
        // A soldier may not invite; a player of a clan is not invited.
        assert_eq!(clans.may_invite(2, 4), Err(ClanError(text::INVITE)));
        assert_eq!(clans.may_invite(1, 3), Err(ClanError(text::IN_CLAN)));
        clans.invite(1, 4, 80, "Stalkers").unwrap();
        // A registered clan lists nobody before the answer.
        assert_eq!(clans.clan(1).unwrap().members.len(), 3);
        let (clan, joined) = clans.answer_invite(4, 1, true).unwrap();
        assert!(joined);
        assert_eq!(clan.members[3].rank, 3);
        // Full clans take nobody.
        clans.set_config(Config {
            max_members: 4,
            ..test_config()
        });
        assert_eq!(clans.may_invite(1, 5), Err(ClanError(text::FULL)));
    }

    #[test]
    fn ranks_go_up_and_down_and_the_clan_changes_hands() {
        let clans = clan_of_three();
        // Up one rank at a time, by the commander.
        let RankUp::Done(clan) = clans.ask_rank_up(1, 2).unwrap() else {
            panic!("no confirmation for a sergeant");
        };
        assert_eq!(clan.member(2).unwrap().rank, 2);
        clans.ask_rank_up(1, 2).unwrap();
        // The deputy (all permissions) acts only on lower ranks, and never
        // lifts one to its own.
        assert_eq!(clans.ask_rank_up(2, 2), Err(ClanError(text::RANK_UP)));
        assert_eq!(clans.ask_rank_up(2, 1), Err(ClanError(text::RANK_UP)));
        let RankUp::Done(clan) = clans.ask_rank_up(2, 3).unwrap() else {
            panic!("a deputy ranks a soldier up");
        };
        assert_eq!(clan.member(3).unwrap().rank, 2);
        assert_eq!(clans.ask_rank_up(2, 3), Err(ClanError(text::RANK_UP)));
        assert_eq!(clans.kick(2, 1), Err(ClanError(text::KICK)));
        // Down, to the lowest and no further.
        assert_eq!(
            clans
                .change_rank(2, 3, false)
                .unwrap()
                .member(3)
                .unwrap()
                .rank,
            3
        );
        assert_eq!(clans.change_rank(2, 3, false), Err(ClanError(text::RANK)));
        // A soldier changes nobody.
        assert_eq!(clans.change_rank(3, 2, false), Err(ClanError(text::RANK)));

        // To the commander's rank: asked first, then the two swap.
        assert_eq!(clans.ask_rank_up(1, 2), Ok(RankUp::Confirm));
        assert_eq!(clans.clan(1).unwrap().leader, 1);
        let clan = clans.change_rank(1, 2, true).unwrap();
        assert_eq!(clan.leader, 2);
        assert_eq!(clan.member(2).unwrap().rank, 0);
        assert_eq!(clan.member(1).unwrap().rank, 1);
        // The old commander disbands nothing now: it leaves.
        assert!(matches!(
            clans.leave_or_disband(1),
            Ok(Departure::Left(clan)) if clan.members.len() == 2
        ));
    }

    #[test]
    fn kicking_needs_the_permission_and_a_lower_rank() {
        let clans = clan_of_three();
        assert_eq!(clans.kick(1, 1), Err(ClanError(text::KICK_SELF)));
        assert_eq!(clans.kick(2, 3), Err(ClanError(text::KICK)));
        assert_eq!(clans.kick(1, 9), Err(ClanError(text::KICK)));
        let clan = clans.kick(1, 3).unwrap();
        assert_eq!(clan.members.len(), 2);
        assert_eq!(clans.clan_of(3), None);
        assert_eq!(clans.tag_of(3), None);
    }

    #[test]
    fn the_commander_edits_the_ranks_and_their_permissions() {
        let clans = clan_of_three();
        assert_eq!(
            clans.set_permission(2, 3, 0, true),
            Err(ClanError(text::PERMISSION))
        );
        // The commander's own rank is fixed; so is a code past 4.
        assert_eq!(
            clans.set_permission(1, 0, 0, false),
            Err(ClanError(text::PERMISSION))
        );
        assert_eq!(
            clans.set_permission(1, 3, 5, true),
            Err(ClanError(text::PERMISSION))
        );
        let clan = clans.set_permission(1, 3, 0, true).unwrap();
        assert_eq!(clan.ranks[3].permissions, permission::INVITE);
        let clan = clans.set_permission(1, 3, 4, true).unwrap();
        assert_eq!(
            clan.ranks[3].permissions,
            permission::INVITE | permission::BATTLE_COMMANDER
        );
        assert!(clans.may_invite(3, 7).is_ok());
        let clan = clans.set_permission(1, 3, 0, false).unwrap();
        assert_eq!(clan.ranks[3].permissions, permission::BATTLE_COMMANDER);

        // A new rank goes above the lowest; the soldiers stay the lowest.
        assert_eq!(
            clans.create_rank(2, "Scouts"),
            Err(ClanError(text::CREATE_RANK))
        );
        assert_eq!(clans.create_rank(1, " "), Err(ClanError(text::CREATE_RANK)));
        assert_eq!(
            clans.create_rank(1, "SOLDIER"),
            Err(ClanError(text::CREATE_RANK))
        );
        let clan = clans.create_rank(1, "Scouts").unwrap();
        let names: Vec<&str> = clan.ranks.iter().map(|rank| rank.name.as_str()).collect();
        assert_eq!(
            names,
            ["commander", "assistant", "warlord", "Scouts", "soldier"]
        );
        assert!(!clan.ranks[3].is_default);
        assert_eq!(clan.member(2).unwrap().rank, 4);
        // No more than ten.
        for index in 0..5 {
            clans.create_rank(1, &format!("Rank {index}")).unwrap();
        }
        assert_eq!(clans.clan(1).unwrap().ranks.len(), MAX_RANKS);
        assert_eq!(
            clans.create_rank(1, "More"),
            Err(ClanError(text::CREATE_RANK))
        );

        // Removing: never a default rank; its members get the next lower.
        assert_eq!(clans.remove_rank(1, 2), Err(ClanError(text::REMOVE_RANK)));
        assert_eq!(clans.remove_rank(1, 30), Err(ClanError(text::REMOVE_RANK)));
        clans.ask_rank_up(1, 2).unwrap();
        assert_eq!(clans.clan(1).unwrap().member(2).unwrap().rank, 8);
        let clan = clans.remove_rank(1, 8).unwrap();
        assert_eq!(clan.ranks.len(), 9);
        assert_eq!(clan.member(2).unwrap().rank, 8);
        assert_eq!(clan.ranks[8].name, "soldier");
        let clan = clans.remove_rank(1, 3).unwrap();
        assert_eq!(clan.member(2).unwrap().rank, 7);
    }

    #[test]
    fn the_daily_message_is_cut_to_the_clients_buffer() {
        let clans = clan_of_three();
        assert_eq!(
            clans.set_daily_message(3, "hello"),
            Err(ClanError(text::DAILY))
        );
        let clan = clans.set_daily_message(1, "Rally\nat\u{7} nine").unwrap();
        assert_eq!(clan.daily_message, "Rally\nat nine");
        let clan = clans.set_daily_message(1, &"я".repeat(400)).unwrap();
        assert_eq!(clan.daily_message.encode_utf16().count(), MAX_DAILY_MESSAGE);
        // A pair of units is never cut in two.
        let clan = clans.set_daily_message(1, &"😀".repeat(200)).unwrap();
        assert_eq!(clan.daily_message.chars().count(), 122);
    }

    #[test]
    fn a_clan_levels_with_its_members_experience() {
        let clans = clan_of_three();
        assert_eq!(clans.clan(1).unwrap().level(), (1, 0, 10_000));
        assert_eq!(clans.add_experience(9, 500), None);
        assert_eq!(clans.add_experience(2, 0), None);
        let (clan, level) = clans.add_experience(2, 9_999).unwrap();
        assert_eq!((clan.experience, level), (9_999, None));
        let (clan, level) = clans.add_experience(3, 1).unwrap();
        assert_eq!(level, Some(2));
        assert_eq!(clan.level(), (2, 10_000, 30_000));
        assert_eq!(level_start(1), 0);
        assert!(level_start(MAX_LEVEL + 1) < u32::MAX);
    }

    #[test]
    fn a_clan_team_forms_and_its_commander_readies_it() {
        let clans = clan_of_three();
        clans.set_config(Config {
            team_min: 2,
            team_max: 2,
            ..test_config()
        });
        // A soldier has no battle command; nobody joins a team that is not.
        assert_eq!(clans.create_team(2), Err(ClanError(text::TEAM_CREATE)));
        assert_eq!(clans.ask_to_join_team(2), Err(ClanError(text::TEAM_JOIN)));
        let team = clans.create_team(1).unwrap();
        assert_eq!(team.commander().unwrap().account_id, 1);
        assert_eq!(
            clans.create_team(1),
            Err(ClanError(text::TEAM_HAS_COMMANDER))
        );

        // An application, taken into the lineup by the commander.
        let team = clans.ask_to_join_team(2).unwrap();
        assert_eq!(team.member(2).unwrap().role, role::APPLICANT);
        assert_eq!(clans.team_of(2), None);
        assert_eq!(clans.ask_to_join_team(2), Err(ClanError(text::TEAM_IN)));
        assert_eq!(
            clans.set_team_status(2, 2, role::SOLDIER),
            Err(ClanError(text::TEAM_STATUS))
        );
        assert_eq!(
            clans.set_team_status(1, 2, role::COMMANDER),
            Err(ClanError(text::TEAM_STATUS))
        );
        let team = clans.set_team_status(1, 2, role::SOLDIER).unwrap();
        assert_eq!(team.member(2).unwrap().role, role::SOLDIER);
        assert_eq!(clans.team_of(2).unwrap().id, team.id);

        // An invite: to the reserve, the lineup being full.
        assert_eq!(
            clans.may_invite_to_team(2, 3),
            Err(ClanError(text::TEAM_INVITE))
        );
        assert_eq!(
            clans.may_invite_to_team(1, 9),
            Err(ClanError(text::TEAM_INVITE))
        );
        assert_eq!(
            clans.may_invite_to_team(1, 2),
            Err(ClanError(text::TEAM_HAS_STATUS))
        );
        clans.invite_to_team(1, 3, 90, "").unwrap();
        assert_eq!(
            clans.answer_team_invite(3, 2, true),
            Err(ClanError(text::TEAM_ADD))
        );
        let team = clans.answer_team_invite(3, 1, true).unwrap().unwrap();
        assert_eq!(team.member(3).unwrap().role, role::RESERVE);
        assert_eq!(
            clans.set_team_status(1, 3, role::SOLDIER),
            Err(ClanError(text::TEAM_FULL))
        );
        // The roles stay grouped: commander, lineup, reserve.
        let roles: Vec<u8> = team.members.iter().map(|member| member.role).collect();
        assert_eq!(roles, [0, 1, 2]);

        // Play: the lineup and the commander; the reserve does not.
        assert_eq!(clans.clan_play(3, 1), Err(ClanError(text::TEAM_PLAY)));
        clans.clan_play(1, 2).unwrap();
        clans.clan_play(2, 3).unwrap();
        // The commander is ready only once every soldier is.
        assert_eq!(
            clans.commander_ready(1, true),
            Err(ClanError(text::TEAM_READY))
        );
        assert_eq!(
            clans.soldier_ready(1, true),
            Err(ClanError(text::TEAM_READY))
        );
        assert_eq!(
            clans.commander_ready(2, true),
            Err(ClanError(text::TEAM_READY))
        );
        clans.soldier_ready(2, true).unwrap();
        let team = clans.commander_ready(1, true).unwrap();
        assert!(team.is_queued());
        assert_eq!(team.member(2).unwrap().kit_id, 3);
        // A soldier's Play is off while the commander is ready.
        assert_eq!(
            clans.soldier_ready(2, false),
            Err(ClanError(text::TEAM_READY))
        );
        // A change of the team takes it out of the queue.
        let team = clans.set_team_status(1, 2, role::RESERVE).unwrap();
        assert!(!team.is_queued());
        // One player is under the minimum of two.
        assert_eq!(
            clans.commander_ready(1, true),
            Err(ClanError(text::TEAM_READY))
        );
        clans.set_team_status(1, 2, role::SOLDIER).unwrap();
        clans.soldier_ready(2, true).unwrap();
        clans.commander_ready(1, true).unwrap();
        // A match was made: nobody is ready any more.
        let team = clans.team_unready(1).unwrap();
        assert!(team.members.iter().all(|member| !member.ready));
        assert_eq!(clans.team_unready(1), None);
    }

    #[test]
    fn a_player_who_goes_takes_its_team_out_of_the_queue() {
        let clans = clan_of_three();
        clans.create_team(1).unwrap();
        clans.invite_to_team(1, 2, 90, "").unwrap();
        clans.answer_team_invite(2, 1, true).unwrap();
        assert_eq!(clans.member_unready(2), None);
        clans.soldier_ready(2, true).unwrap();
        clans.commander_ready(1, true).unwrap();
        let team = clans.member_unready(2).unwrap();
        assert!(!team.is_queued());
        assert!(!team.member(2).unwrap().ready);
        assert_eq!(clans.member_unready(9), None);
        // A declined invite changes nothing.
        clans.invite_to_team(1, 3, 91, "").unwrap();
        assert_eq!(clans.answer_team_invite(3, 1, false), Ok(None));
    }

    #[test]
    fn the_command_of_a_team_changes_hands() {
        let clans = clan_of_three();
        clans.set_permission(1, 3, 4, true).unwrap();
        // Member 2 (a soldier who may command now) makes the team.
        clans.create_team(2).unwrap();
        clans.invite_to_team(2, 3, 90, "").unwrap();
        clans.answer_team_invite(3, 2, true).unwrap();
        // Ousting: the commander itself or the clan's commander.
        assert_eq!(clans.oust_commander(3), Err(ClanError(text::TEAM_OUST)));
        let team = clans.oust_commander(1).unwrap();
        assert_eq!(team.commander(), None);
        assert_eq!(team.member(2).unwrap().role, role::SOLDIER);
        assert_eq!(clans.oust_commander(1), Err(ClanError(text::TEAM_OUST)));
        // The command is free: who may command takes it.
        let team = clans.create_team(3).unwrap();
        assert_eq!(team.members[0].account_id, 3);
        assert_eq!(team.members.len(), 2);
        // The clan's commander names another.
        assert_eq!(
            clans.set_team_commander(3, 2),
            Err(ClanError(text::TEAM_COMMANDER))
        );
        assert_eq!(
            clans.set_team_commander(1, 1),
            Err(ClanError(text::TEAM_COMMANDER))
        );
        let team = clans.set_team_commander(1, 2).unwrap();
        assert_eq!(team.members[0].account_id, 2);
        assert_eq!(team.member(3).unwrap().role, role::SOLDIER);
        // Removing and leaving.
        assert_eq!(
            clans.remove_team_member(3, 2),
            Err(ClanError(text::TEAM_REMOVE))
        );
        assert_eq!(clans.remove_team_member(2, 3).unwrap().members.len(), 1);
        assert_eq!(clans.leave_team(3), Err(ClanError(text::TEAM_LEAVE)));
        // The last one out: no team.
        assert_eq!(clans.leave_team(2), Ok((1, None)));
        assert_eq!(clans.team(1), None);
        // Disbanding: the team's commander or the clan's.
        clans.create_team(2).unwrap();
        clans.ask_to_join_team(3).unwrap();
        assert_eq!(clans.disband_team(3), Err(ClanError(text::TEAM_DISBAND)));
        assert_eq!(clans.disband_team(1).unwrap().members.len(), 2);
        assert_eq!(clans.disband_team(1), Err(ClanError(text::TEAM_DISBAND)));
    }

    #[test]
    fn leaving_the_clan_leaves_its_team() {
        let clans = clan_of_three();
        clans.create_team(1).unwrap();
        clans.invite_to_team(1, 2, 90, "").unwrap();
        clans.answer_team_invite(2, 1, true).unwrap();
        clans.kick(1, 2).unwrap();
        assert_eq!(clans.team(1).unwrap().members.len(), 1);
        // Disbanding the clan ends the team.
        assert!(matches!(
            clans.leave_or_disband(1),
            Ok(Departure::Disbanded(_))
        ));
        assert_eq!(clans.team(1), None);
        assert_eq!(clans.clan_of(3), None);
    }

    #[test]
    fn changes_are_handed_to_the_repository_once() {
        let clans = clan_of_three();
        let (saved, deleted) = clans.take_changes();
        assert_eq!(saved.len(), 1);
        assert!(saved[0].registered && deleted.is_empty());
        assert_eq!(clans.take_changes(), (Vec::new(), Vec::new()));
        clans.set_daily_message(1, "hi").unwrap();
        // A failed write is tried again.
        let (saved, _) = clans.take_changes();
        clans.mark_unsaved(saved[0].id, false);
        assert_eq!(clans.take_changes().0, saved);

        // What a repository kept comes back with its members' clans.
        let clans = Clans::loaded(saved);
        assert_eq!(clans.clan_of(3).unwrap().daily_message, "hi");
        assert_eq!(clans.tag_of(2).as_deref(), Some("STK"));
        assert_eq!(clans.take_changes(), (Vec::new(), Vec::new()));
        // A new clan gets the next id.
        assert_eq!(clans.reserve(7, "Others", "OTH").unwrap().id, 2);
    }

    #[test]
    fn every_refusal_is_a_text_of_the_game() {
        // The 0.26g English localization dump (the toolkit's `dbgrab` makes
        // it from the game's `resources.db`), when `SRP_026G_LOCALIZATION`
        // names it on this machine.
        let Ok(dump) = std::env::var("SRP_026G_LOCALIZATION") else {
            return;
        };
        let Ok(localization) = std::fs::read_to_string(dump) else {
            return;
        };
        for key in TEXT_KEYS {
            assert!(localization.contains(&format!("{key}:")), "{key}");
        }
    }
}
