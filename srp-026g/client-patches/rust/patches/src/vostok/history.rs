//! Bullet-history suffix access. A read commits its native cursor only
//! after the complete typed value has been decoded successfully.
use super::{Context, parse::Reader, project::PhysicsId};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryError { InvalidRange, InvalidPayload }

pub struct HistoryWriter<'t> {
    writer: raw::BufferWriter,
    physics: PhysicsId,
    _context: &'t Context<'t>,
}
impl<'t> HistoryWriter<'t> {
    pub fn at_saved(ctx: &'t Context<'t>) -> Option<Self> {
        use raw::site::bullet_history_saved as site;
        // SAFETY: native serialize's epilogue retains EDI=manager and
        // EBP's caller-frame writer argument until it returns.
        unsafe {
            Some(Self { writer: site::frame(ctx)?.writer()?,
                        physics: PhysicsId::of(site::manager(ctx)?.physics()?), _context: ctx })
        }
    }
    pub fn physics(&self) -> PhysicsId { self.physics }
    pub fn append(&mut self, words: impl IntoIterator<Item = u32>) {
        for word in words {
            // SAFETY: native writer is alive for the serialization hook;
            // write_u32 copies this local before returning. Diagnostic
            // strings are static and terminated.
            unsafe {
                raw::engine_write_u32(core::ptr::from_ref(&word).cast(), Some(self.writer),
                    c"lamps.rs".as_ptr().cast(), 0, c"lamps::save".as_ptr().cast(), c"lamps".as_ptr().cast());
            }
        }
    }
}

pub struct HistoryReader<'t> {
    reader: raw::BufferReader,
    physics: PhysicsId,
    _context: &'t Context<'t>,
}
impl<'t> HistoryReader<'t> {
    pub fn at_loaded(ctx: &'t Context<'t>) -> Option<Self> {
        use raw::site::bullet_history_loaded as site;
        // SAFETY: native deserialize's epilogue retains EDI=manager and
        // EBX=reader. The reader's source bytes remain alive for this hook.
        unsafe {
            Some(Self { reader: site::reader(ctx)?, physics: PhysicsId::of(site::manager(ctx)?.physics()?), _context: ctx })
        }
    }
    pub fn physics(&self) -> PhysicsId { self.physics }
    pub fn decode<T>(&mut self, parse: impl FnOnce(&mut Reader<'_>) -> Option<T>) -> Result<T, HistoryError> {
        // SAFETY: native caller owns the buffer. Its {begin,cursor,size}
        // range is checked before creating a slice or advancing any pointer.
        unsafe {
            let begin = self.reader.begin() as usize;
            let cursor = self.reader.cursor();
            let size = self.reader.size().read() as usize;
            let end = begin.checked_add(size).ok_or(HistoryError::InvalidRange)?;
            let at = cursor as usize;
            if cursor.is_null() || at < begin || at > end { return Err(HistoryError::InvalidRange); }
            let available = end - at;
            let mut source = Reader::of_memory(cursor, available);
            let value = parse(&mut source).ok_or(HistoryError::InvalidPayload)?;
            self.reader.set_cursor(cursor.add(available - source.remaining().len()));
            Ok(value)
        }
    }
}
