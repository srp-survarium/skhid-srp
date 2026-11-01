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
