// A bounded text filter before DEFLATE. It never emits an incomplete line, so
// markers split across source reads cannot leak a prefix of a sensitive record.
use crate::zip::Source;

pub const ERROR_UNSAFE_TEXT: u32 = 0xE000_0201;
pub const ERROR_LONG_LINE: u32 = 0xE000_0202;
pub const ERROR_AMBIGUOUS_RECORD: u32 = 0xE000_0203;
const LINE: usize = 64 * 1024;
const INPUT: usize = 8 * 1024;
const REMOVED: &[u8] = b"[SRP: sensitive log record removed]\r\n";

pub struct Filter<'a> {
    source: &'a mut dyn Source,
    input: [u8; INPUT],
    input_at: usize,
    input_len: usize,
    line: [u8; LINE],
    output_at: usize,
    output_len: usize,
    ended: bool,
    suppress: bool,
    pub removed_lines: u64,
    pub emitted: u64,
}
impl<'a> Filter<'a> {
    pub fn new(source: &'a mut dyn Source) -> Self {
        Self {
            source,
            input: [0; INPUT],
            input_at: 0,
            input_len: 0,
            line: [0; LINE],
            output_at: 0,
            output_len: 0,
            ended: false,
            suppress: false,
            removed_lines: 0,
            emitted: 0,
        }
    }
    fn next_line(&mut self) -> Result<bool, u32> {
        let mut len = 0;
        loop {
            if self.input_at == self.input_len {
                if self.ended {
                    break;
                }
                self.input_len = self.source.read(&mut self.input)?;
                self.input_at = 0;
                if self.input_len == 0 {
                    self.ended = true;
                    break;
                }
            }
            let byte = self.input[self.input_at];
            self.input_at += 1;
            if len == LINE {
                return Err(ERROR_LONG_LINE);
            }
            self.line[len] = byte;
            len += 1;
            if byte == b'\n' {
                break;
            }
        }
        if len == 0 {
            return Ok(false);
        }
        let line = &self.line[..len];
        // Fail closed on UTF-16/32, binary/control encodings and non-UTF-8 text.
        // Never zip a bytewise "redaction" of an encoding we cannot recognize.
        if core::str::from_utf8(line).is_err()
            || line
                .iter()
                .any(|&b| b == 127 || (b < 32 && !b"\r\n\t".contains(&b)))
        {
            return Err(ERROR_UNSAFE_TEXT);
        }
        let sensitive = sensitive(line);
        if sensitive && ambiguous(line) {
            return Err(ERROR_AMBIGUOUS_RECORD);
        }
        if sensitive || (self.suppress && !record_start(line)) {
            self.removed_lines += 1;
            self.output_len = if sensitive {
                self.line[..REMOVED.len()].copy_from_slice(REMOVED);
                REMOVED.len()
            } else {
                0
            };
            self.suppress = true;
        } else {
            self.suppress = false;
            self.output_len = len;
        }
        self.output_at = 0;
        Ok(true)
    }
}
impl Source for Filter<'_> {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, u32> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.output_at == self.output_len {
            if !self.next_line()? {
                return Ok(0);
            }
        }
        let n = out.len().min(self.output_len - self.output_at);
        out[..n].copy_from_slice(&self.line[self.output_at..self.output_at + n]);
        self.output_at += n;
        self.emitted += n as u64;
        Ok(n)
    }
}
fn starts(text: &[u8], pattern: &[u8]) -> bool {
    text.get(..pattern.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(pattern))
}
fn sensitive(line: &[u8]) -> bool {
    for (i, &b) in line.iter().enumerate() {
        let tail = &line[i..];
        let patterns: &[&[u8]] = match b.to_ascii_lowercase() {
            b'p' => &[
                b"password",
                b"passwd",
                b"pwd",
                b"passphrase",
                b"private_key",
                b"private-key",
            ],
            b'a' => &[
                b"account_digest",
                b"account-digest",
                b"accountdigest",
                b"authorization",
                b"api_key",
                b"api-key",
                b"api key",
                b"apikey",
                b"access_key",
                b"access-key",
                b"access key",
            ],
            b'b' => &[b"bearer", b"basic "],
            b't' => &[b"token"],
            b's' => &[b"secret", b"session_key", b"session-key", b"session key"],
            b'c' => &[
                b"cookie",
                b"credential",
                b"command line",
                b"command_line",
                b"command-line",
                b"commandline",
                b"cmdline",
                b"cmd_line",
                b"cmd-line",
            ],
            _ => &[],
        };
        if patterns.iter().any(|p| starts(tail, p)) {
            return true;
        }
        // URL user-info: scheme://user:password@host, with no credential label.
        if starts(tail, b"://") {
            let authority = &tail[3..];
            let end = authority
                .iter()
                .position(|b| b"/ \t\r\n".contains(b))
                .unwrap_or(authority.len());
            if authority[..end].contains(&b'@') {
                return true;
            }
        }
    }
    false
}
fn ambiguous(line: &[u8]) -> bool {
    let mut quote = 0;
    let mut escaped = false;
    for &b in line {
        if escaped {
            escaped = false;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            continue;
        }
        if quote == 0 && (b == b'\'' || b == b'"') {
            quote = b;
        } else if b == quote {
            quote = 0;
        }
    }
    let last = line.iter().rfind(|b| !b" \t\r\n".contains(b));
    quote != 0 || last == Some(&b'\\')
}
fn digits(text: &[u8]) -> bool {
    text.iter().all(u8::is_ascii_digit)
}
fn clock(text: &[u8]) -> bool {
    text.len() == 12
        && text[2] == b':'
        && text[5] == b':'
        && text[8] == b'.'
        && digits(&text[..2])
        && digits(&text[3..5])
        && digits(&text[6..8])
        && digits(&text[9..])
}
fn record_start(line: &[u8]) -> bool {
    // Updater: YYYY-MM-DD HH:MM:SS + space.
    if line.len() >= 20
        && line[4] == b'-'
        && line[7] == b'-'
        && line[10] == b' '
        && line[13] == b':'
        && line[16] == b':'
        && line[19] == b' '
        && digits(&line[..4])
        && digits(&line[5..7])
        && digits(&line[8..10])
        && digits(&line[11..13])
        && digits(&line[14..16])
        && digits(&line[17..19])
    {
        return true;
    }
    // Native: core <info>\t[HH:MM:SS.mmm]\t... (channel/thread variants too).
    let Some(at) = line.iter().take(80).position(|&b| b == b'<') else {
        return false;
    };
    if at == 0
        || line[at - 1] != b' '
        || !line[..at - 1]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(b))
    {
        return false;
    }
    let Some(end) = line[at + 1..].iter().take(16).position(|&b| b == b'>') else {
        return false;
    };
    let end = at + 1 + end;
    if end == at + 1 || !line[at + 1..end].iter().all(u8::is_ascii_alphabetic) {
        return false;
    }
    let rest = &line[end + 1..];
    rest.len() >= 16 && rest[..2] == *b"\t[" && clock(&rest[2..14]) && rest[14..16] == *b"]\t"
}
