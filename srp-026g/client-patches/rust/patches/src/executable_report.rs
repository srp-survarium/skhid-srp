//! Hash and, when requested, send only the running executable. File ownership
//! lasts one match connection; close/error/completion all drop it. Hashing and
//! transfer are bounded per frame, with no catch-up burst after a stall.
#![forbid(unsafe_code)]
use sign_in_cipher::Sha256;
use srp_validate::executable::{Answer, Carrier, Identity, Reply, Upload, CHUNK, MAX_FILE};
use crate::vostok::{Context, Flow, NoExit};

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::executable::loaded(ctx);
    Flow::Continue
}
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::executable::frame(ctx);
    Flow::Continue
}
pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::executable::close(ctx);
    Flow::Continue
}

pub trait Source {
    fn read(&mut self, into: &mut [u8]) -> Option<usize>;
    fn seek(&mut self, offset: u32) -> bool;
}

enum State<F> {
    Hashing { file: F, hash: Sha256, size: u32 },
    Offering { file: F, identity: Identity, last: Option<u32> },
    Sending { file: F, identity: Identity, offset: u32, end: u32, last: Option<u32> },
    Done,
}
pub struct Client<F> { state: State<F> }
impl<F: Source> Client<F> {
    pub fn new(file: F) -> Self { Self { state: State::Hashing { file, hash: Sha256::new(), size: 0 } } }
    pub fn reply(&mut self, reply: Reply) {
        let old = core::mem::replace(&mut self.state, State::Done);
        self.state = match old {
            State::Offering { mut file, identity, last } if reply.sha256 == identity.sha256 => match reply.answer {
                Answer::Need(offset) => match identity.window_end(offset) {
                    Some(end) if file.seek(offset) => State::Sending { file, identity, offset, end, last },
                    _ => State::Done,
                },
                Answer::Stored | Answer::Refused => State::Done,
            },
            // Requests are retried in answer to an offer. A delayed request
            // cannot rewind a window while its packets are being queued.
            State::Sending { identity, .. } if reply.sha256 == identity.sha256 && matches!(reply.answer, Answer::Stored | Answer::Refused) => State::Done,
            state => state,
        };
    }
    pub fn tick(&mut self, now: u32, match_id: u32, mut send: impl FnMut(Carrier<'_>) -> bool) {
        let old = core::mem::replace(&mut self.state, State::Done);
        self.state = match old {
            State::Hashing { mut file, mut hash, mut size } => {
                let mut bytes = [0; 8192];
                match file.read(&mut bytes).filter(|&n| n <= bytes.len()) {
                    Some(0) => match Identity::new(size, hash.finish()) {
                        Some(identity) => State::Offering { file, identity, last: None },
                        None => State::Done,
                    },
                    Some(n) if size <= MAX_FILE.saturating_sub(n as u32) => {
                        hash.update(&bytes[..n]); size += n as u32;
                        State::Hashing { file, hash, size }
                    }
                    _ => State::Done,
                }
            }
            State::Offering { file, identity, last } => {
                if last.is_none_or(|last| now.wrapping_sub(last) >= 1000)
                    && send(Carrier { match_id, upload: Upload::Offer(identity) }) {
                    State::Offering { file, identity, last: Some(now) }
                } else { State::Offering { file, identity, last } }
            }
            State::Sending { mut file, identity, mut offset, end, last } => {
                if last.is_some_and(|last| now.wrapping_sub(last) < 25) {
                    State::Sending { file, identity, offset, end, last }
                } else {
                    // Four chunks per 25 ms: at most 64 kB/s of file data,
                    // and only one 6.4 kB window outstanding at a time.
                    let mut ok = true;
                    for _ in 0..4 {
                        if offset == end { break; }
                        let mut bytes = [0; CHUNK];
                        let wanted = identity.chunk_len(offset).unwrap_or(0);
                        if !file.seek(offset) { ok = false; break; }
                        let mut got = 0;
                        while got < wanted {
                            match file.read(&mut bytes[got..wanted]) {
                                Some(n) if n > 0 && n <= wanted - got => got += n,
                                _ => { ok = false; break; }
                            }
                        }
                        if !ok { break; }
                        if !send(Carrier { match_id, upload: Upload::Data { sha256: identity.sha256, offset, bytes: &bytes[..wanted] } }) { break; }
                        offset += wanted as u32;
                    }
                    if !ok { State::Done }
                    else if offset == end { State::Offering { file, identity, last: Some(now) } }
                    else { State::Sending { file, identity, offset, end, last: Some(now) } }
                }
            }
            State::Done => State::Done,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{rc::Rc, cell::Cell, vec::Vec};
    struct File { bytes: Vec<u8>, at: usize, dropped: Rc<Cell<bool>> }
    impl Source for File {
        fn read(&mut self, into: &mut [u8]) -> Option<usize> {
            let n = into.len().min(self.bytes.len() - self.at).min(137); // short reads
            into[..n].copy_from_slice(&self.bytes[self.at..self.at+n]); self.at += n; Some(n)
        }
        fn seek(&mut self, offset: u32) -> bool { self.at = offset as usize; self.at <= self.bytes.len() }
    }
    impl Drop for File { fn drop(&mut self) { self.dropped.set(true); } }
    #[test]
    fn hash_offer_paced_upload_backpressure_and_completion() {
        let bytes: Vec<_> = (0..9000).map(|n| n as u8).collect();
        let dropped = Rc::new(Cell::new(false));
        let mut client = Client::new(File { bytes: bytes.clone(), at: 0, dropped: dropped.clone() });
        let mut identity = None;
        for now in 0..100 { client.tick(now, 7, |c| { if let Upload::Offer(i) = c.upload { identity = Some(i); } true }); }
        let identity = identity.unwrap();
        let mut hash = Sha256::new(); hash.update(&bytes); assert_eq!(identity.sha256, hash.finish());
        let mut received = Vec::new();
        let mut now = u32::MAX - 50;
        for start in [0, srp_validate::executable::WINDOW] {
            client.reply(Reply { match_id: 7, sha256: identity.sha256, answer: Answer::Need(start) });
            client.tick(now, 7, |_| false); // no packet room: no skipped bytes
            let end = identity.window_end(start).unwrap();
            while received.len() < end as usize {
                now = now.wrapping_add(25);
                let mut count = 0;
                client.tick(now, 7, |c| { if let Upload::Data { offset, bytes, .. } = c.upload {
                    assert_eq!(offset as usize, received.len()); received.extend_from_slice(bytes); count += 1;
                } true });
                assert!(count <= 4);
                client.tick(now, 7, |_| panic!("no same-frame burst"));
            }
        }
        assert_eq!(received, bytes);
        client.reply(Reply { match_id: 7, sha256: identity.sha256, answer: Answer::Stored });
        assert!(dropped.get());
        client.tick(now.wrapping_add(9999), 7, |_| panic!("done"));
    }
}
