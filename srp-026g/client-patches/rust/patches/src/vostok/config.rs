//! Read-only binary configuration views. Discriminants are decoded once;
//! patch code receives typed values, never a record or an address.
use core::{ffi::CStr, marker::PhantomData, num::NonZeroU16};
use crate::raw;
use super::Thread;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError { Missing, WrongType, MissingChildren }

pub enum ConfigValue<'t> {
    Integer(u32),
    Number(f32),
    Table(ConfigTable<'t>),
    Array(ConfigEntries<'t>),
    /// A text (type 5): its bytes, without the zero.
    Text(&'t CStr),
    Unsupported,
}

impl<'t> ConfigValue<'t> {
    /// The binary config resource is alive and immutable during this hook;
    /// its loader has relocated every node and child array.
    pub(super) unsafe fn read(node: raw::EngineCfg) -> Result<Self, ConfigError> {
        // SAFETY: the caller owns the resource for this view's lifetime.
        unsafe {
            Ok(match node.node_type().read() {
                1 => Self::Integer(node.data().read()),
                2 => Self::Number(f32::from_bits(node.data().read())),
                kind @ (3 | 4) => {
                    let entries = match NonZeroU16::new(node.count().read()) {
                        None => Entries::Empty,
                        Some(count) => Entries::Populated {
                            first: node.children().ok_or(ConfigError::MissingChildren)?, count,
                            lifetime: PhantomData,
                        },
                    };
                    let entries = ConfigEntries(entries);
                    if kind == 3 { Self::Table(ConfigTable { node, entries }) } else { Self::Array(entries) }
                }
                // a text's count is its bytes with the zero: the string is
                // read no further than that.
                5 => super::ui::c_string(node.text(), usize::from(node.count().read()))
                    .map_or(Self::Unsupported, Self::Text),
                _ => Self::Unsupported,
            })
        }
    }
    pub fn integer(self) -> Result<u32, ConfigError> {
        match self { Self::Integer(n) => Ok(n), _ => Err(ConfigError::WrongType) }
    }
    pub fn number(self) -> Result<f32, ConfigError> {
        match self {
            Self::Integer(n) => Ok((n as i32) as f32), Self::Number(n) => Ok(n),
            _ => Err(ConfigError::WrongType),
        }
    }
    pub fn text(self) -> Result<&'t CStr, ConfigError> {
        match self { Self::Text(text) => Ok(text), _ => Err(ConfigError::WrongType) }
    }
    pub fn table(self) -> Result<ConfigTable<'t>, ConfigError> {
        match self { Self::Table(table) => Ok(table), _ => Err(ConfigError::WrongType) }
    }
    pub fn entries(self) -> Result<ConfigEntries<'t>, ConfigError> {
        match self {
            Self::Table(table) => Ok(table.entries), Self::Array(entries) => Ok(entries),
            _ => Err(ConfigError::WrongType),
        }
    }
}

pub struct ConfigTable<'t> { node: raw::EngineCfg, entries: ConfigEntries<'t> }

impl<'t> ConfigTable<'t> {
    pub fn get(&self, name: &CStr) -> Result<Option<ConfigValue<'t>>, ConfigError> {
        // SAFETY: this node was parsed as a table; its binary config lives
        // for 't and name is a terminated string for both native calls.
        unsafe {
            if raw::engine_cfg_has(Some(self.node), name.as_ptr().cast()) == 0 { return Ok(None); }
            let child = raw::engine_cfg_child(Some(self.node), name.as_ptr().cast()).ok_or(ConfigError::Missing)?;
            ConfigValue::read(child).map(Some)
        }
    }
    pub fn required(&self, name: &CStr) -> Result<ConfigValue<'t>, ConfigError> {
        self.get(name)?.ok_or(ConfigError::Missing)
    }
    pub fn entries(self) -> ConfigEntries<'t> { self.entries }
}

pub struct ConfigEntries<'t>(Entries<'t>);

enum Entries<'t> {
    Empty,
    Populated { first: raw::EngineCfg, count: NonZeroU16, lifetime: PhantomData<&'t Thread> },
}

impl<'t> ConfigEntries<'t> {
    pub fn len(&self) -> usize {
        match &self.0 { Entries::Empty => 0, Entries::Populated { count, .. } => usize::from(count.get()) }
    }
    pub fn is_empty(&self) -> bool { matches!(self.0, Entries::Empty) }
    pub fn iter(&self) -> impl Iterator<Item = Result<ConfigValue<'t>, ConfigError>> + '_ {
        (0..self.len()).map(|i| {
            let Entries::Populated { first, .. } = &self.0 else { unreachable!() };
            // SAFETY: i is inside the parsed child count; the config owns
            // the relocated contiguous records and outlives the iterator.
            unsafe { ConfigValue::read(first.at(i)) }
        })
    }
}

/// Where a value of a config is, from a table: one of its keys, or each
/// entry of an array (or table) there.
#[derive(Clone, Copy)]
pub enum Step<'a> { Key(&'a CStr), Each }

/// A project's client_project as its cook gets it (`project.cook`,
/// project_cooker_simple::create_game_objects at its entry): loaded and
/// relocated, read by nothing yet, so the cook and every later reader (the
/// project's own `config`, patch 81's lamps at its insertion) see only what
/// is left. Entries are taken out as the game's lists hold them: one
/// contiguous run of records and a count.
pub struct ConfigEdit<'t> { root: raw::EngineCfg, _thread: &'t Thread }

impl<'t> ConfigEdit<'t> {
    pub fn at_project_cook(ctx: &'t super::Context<'_>) -> Option<Self> {
        use raw::site::project_cook as site;
        // SAFETY: the site is create_game_objects' entry: its second argument
        // is the project's binary config, held (counted) by the cook for the
        // call; nothing has read a record of it yet.
        let root = unsafe { site::frame(ctx)?.config()?.root()? };
        Some(Self { root, _thread: ctx.thread() })
    }

    /// The config, read.
    pub fn root(&self) -> Result<ConfigTable<'_>, ConfigError> {
        // SAFETY: the cook holds the config for the hook; this view borrows
        // the edit, so no record moves while it is read.
        unsafe { ConfigValue::read(self.root) }?.table()
    }

    /// The root's list `list` (an array or a table), if the config has it.
    fn list(&self, list: &CStr) -> Option<raw::EngineCfg> {
        // SAFETY: as root; the key is a terminated string for both native calls.
        unsafe {
            if raw::engine_cfg_has(Some(self.root), list.as_ptr().cast()) == 0 { return None; }
            raw::engine_cfg_child(Some(self.root), list.as_ptr().cast())
        }
    }

    /// Takes out of the root's list `list` every entry `take` names (by its
    /// index), the later ones moved down in place and the count lowered; the
    /// number taken. A config without the list: none.
    pub fn remove(&mut self, list: &CStr, take: impl Fn(usize) -> bool) -> Result<usize, ConfigError> {
        let Some(node) = self.list(list) else { return Ok(0) };
        // SAFETY: the list's records are its contiguous children, count of
        // them (the loader relocated the array); a record is moved whole
        // (raw::EngineCfg::SIZE bytes) onto one taken or moved before it, and
        // nothing holds a record of this config while the cook has not begun.
        unsafe {
            if !matches!(node.node_type().read(), 3 | 4) { return Err(ConfigError::WrongType); }
            let count = usize::from(node.count().read());
            if count == 0 { return Ok(0); }
            let first = node.children().ok_or(ConfigError::MissingChildren)?;
            let mut kept = 0;
            for i in 0..count {
                if take(i) { continue; }
                if kept != i {
                    core::ptr::copy(first.at(i).data().cast::<u8>(), first.at(kept).data().cast::<u8>(),
                                    raw::EngineCfg::SIZE);
                }
                kept += 1;
            }
            node.count().write(kept as u16);
            Ok(count - kept)
        }
    }

    /// Each integer at `path` from the root becomes `f` of it (a key a table
    /// lacks: nothing there).
    pub fn update_integers<E>(&mut self, path: &[Step<'_>], f: &mut impl FnMut(u32) -> Result<u32, E>)
        -> Result<(), UpdateError<E>> {
        // SAFETY: as root; only an integer's value bits are written.
        unsafe { update(self.root, path, f) }
    }
}

/// Why integers could not be updated: the config's shape, or `f`'s refusal.
#[derive(Debug)]
pub enum UpdateError<E> { Config(ConfigError), Refused(E) }

/// # Safety
/// `node` is a record of a config the caller may write.
unsafe fn update<E>(node: raw::EngineCfg, path: &[Step<'_>], f: &mut impl FnMut(u32) -> Result<u32, E>)
    -> Result<(), UpdateError<E>> {
    unsafe {
        let Some((step, rest)) = path.split_first() else {
            if node.node_type().read() != 1 { return Err(UpdateError::Config(ConfigError::WrongType)); }
            let value = f(node.data().read()).map_err(UpdateError::Refused)?;
            node.data().write(value);
            return Ok(());
        };
        if !matches!(node.node_type().read(), 3 | 4) { return Err(UpdateError::Config(ConfigError::WrongType)); }
        match step {
            Step::Key(key) => {
                if raw::engine_cfg_has(Some(node), key.as_ptr().cast()) == 0 { return Ok(()); }
                let child = raw::engine_cfg_child(Some(node), key.as_ptr().cast())
                    .ok_or(UpdateError::Config(ConfigError::Missing))?;
                update(child, rest, f)
            }
            Step::Each => {
                let count = usize::from(node.count().read());
                if count == 0 { return Ok(()); }
                let first = node.children().ok_or(UpdateError::Config(ConfigError::MissingChildren))?;
                for i in 0..count { update(first.at(i), rest, f)?; }
                Ok(())
            }
        }
    }
}
