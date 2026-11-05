// Host-only harness; the production filter has no OS or allocator dependencies.
mod zip {
    pub trait Source {
        fn read(&mut self, out: &mut [u8]) -> Result<usize, u32>;
    }
}
#[path = "../src/sanitize.rs"]
mod sanitize;
use zip::Source;
struct Input<'a> {
    bytes: &'a [u8],
    step: usize,
}
impl Source for Input<'_> {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, u32> {
        let n = out.len().min(self.step).min(self.bytes.len());
        out[..n].copy_from_slice(&self.bytes[..n]);
        self.bytes = &self.bytes[n..];
        Ok(n)
    }
}
fn filter(bytes: &[u8], step: usize) -> Result<(Vec<u8>, u64), u32> {
    let mut input = Input { bytes, step };
    let mut filter = sanitize::Filter::new(&mut input);
    let mut output = Vec::new();
    let mut buf = [0; 73];
    loop {
        let n = filter.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.extend_from_slice(&buf[..n]);
    }
    assert_eq!(output.len() as u64, filter.emitted);
    Ok((output, filter.removed_lines))
}
const SAFE: &str = "core <info>\t[01:02:03.456]\tmanaged arena = 281.75 MiB\r\n";
#[test]
fn native_command_lines_and_all_labels_across_read_boundaries() {
    for label in [
        "command line : -account_password=",
        "PASSWORD:",
        "account_digest=",
        "Authorization: Bearer ",
        "access_token=",
        "pwd=",
        "api_key=",
        "API-KEY=",
        "passphrase=",
        "private-key=",
        "session key=",
        "command-line=",
        "Cookie: ",
        "https://user:SYNTHETIC@host/",
    ] {
        let text = format!(
            "{SAFE}core <info>\t[01:02:03.456]\t{label}SYNTHETIC\r\ncontinued-SYNTHETIC\r\n{SAFE}"
        );
        for step in [1, 2, 7, 8191, 8192] {
            let (output, removed) = filter(text.as_bytes(), step).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(!output.contains("SYNTHETIC"), "{label}");
            assert_eq!(output.matches("managed arena").count(), 2);
            assert_eq!(removed, 2);
        }
    }
}
#[test]
fn updater_boundary_resumes_and_unframed_continuations_stay_removed() {
    let (out, removed) = filter(b"2026-10-18 12:00:00 password=\nSYNTHETIC\n\n<info> fake\n2026-10-18 12:00:01 budget=512\n", 7).unwrap();
    assert_eq!(removed, 4);
    assert!(out.ends_with(b"budget=512\n"));
    assert!(!out.windows(9).any(|b| b == b"SYNTHETIC"));
}
#[test]
fn unsafe_encodings_long_lines_and_multiline_quotes_fail_closed() {
    for text in [
        b"safe\npassword=\xff".as_slice(),
        b"p\0a\0s\0s\0",
        b"safe\n\x1bpassword",
        b"\xef\xbb\xbf\0",
    ] {
        assert_eq!(filter(text, 3), Err(sanitize::ERROR_UNSAFE_TEXT));
    }
    assert_eq!(
        filter(&vec![b'x'; 65537], 8192),
        Err(sanitize::ERROR_LONG_LINE)
    );
    for text in [
        b"password=\"SYNTHETIC\ncontinued\"\n".as_slice(),
        b"password=SYNTHETIC\\\ncontinued\n",
        b"password='SYNTHETIC\n",
    ] {
        assert_eq!(filter(text, 2), Err(sanitize::ERROR_AMBIGUOUS_RECORD));
    }
}
#[test]
fn safe_utf8_empty_and_final_unterminated_lines_are_exact() {
    for text in ["", "arena 512", "українська\r\nmanaged budget\n", SAFE] {
        assert_eq!(
            filter(text.as_bytes(), 1).unwrap(),
            (text.as_bytes().to_vec(), 0)
        );
    }
}
#[test]
fn failure_after_sensitive_record_is_not_hidden_by_dropped_output() {
    struct Failing(bool);
    impl Source for Failing {
        fn read(&mut self, out: &mut [u8]) -> Result<usize, u32> {
            if self.0 {
                return Err(1234);
            }
            self.0 = true;
            let line = b"password=SYNTHETIC\n";
            out[..line.len()].copy_from_slice(line);
            Ok(line.len())
        }
    }
    let mut source = Failing(false);
    let mut filter = sanitize::Filter::new(&mut source);
    let mut out = [0; 1024];
    assert!(filter.read(&mut out).unwrap() > 0);
    assert_eq!(filter.read(&mut out), Err(1234));
}
#[test]
fn a_colon_before_the_milliseconds_also_frames_a_record() {
    // [HH:MM:SS:mmm] frames a record as [HH:MM:SS.mmm] does: after a removed
    // one, the next record is kept, the unframed continuation is not.
    let text = b"game <INFO>\t[12:00:01:000]\tlogin password=SYNTHETIC\r\ncontinued-SYNTHETIC\r\ngame <INFO>\t[12:00:02:000]\tloading level\r\n";
    let (out, removed) = filter(text, 5).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert_eq!(removed, 2);
    assert!(!out.contains("SYNTHETIC"), "{out}");
    assert!(
        out.ends_with("game <INFO>\t[12:00:02:000]\tloading level\r\n"),
        "{out}"
    );
    // Not a clock: other separators, or not digits.
    let fake = b"game <INFO>\t[12:00:01:000]\tpassword=SYNTHETIC\r\nx <INFO>\t[12:00:02,000]\tSYNTHETIC\r\n";
    let (out, _) = filter(fake, 5).unwrap();
    assert!(!out.windows(9).any(|b| b == b"SYNTHETIC"));
}
#[test]
fn a_tail_starts_at_a_record_never_inside_a_continuation() {
    // A tail cut inside a sensitive record's continuation lines.
    let tail = b"continued-SYNTHETIC\r\nmore-SYNTHETIC\r\ngame <INFO>\t[12:00:02.000]\tloading level\r\nrender <ERROR>\t[12:00:03.000]\tmissing\r\n";
    let from = sanitize::from_record(tail);
    assert!(from.starts_with(b"game <INFO>"));
    let (out, _) = filter(from, 7).unwrap();
    assert!(!out.windows(9).any(|b| b == b"SYNTHETIC"));
    assert!(out.ends_with(b"missing\r\n"));
    // Updater records count too; nothing framed at all gives nothing.
    assert!(sanitize::from_record(b"x\n2026-10-18 12:00:00 start\n").starts_with(b"2026-"));
    assert!(sanitize::from_record(b"only\ncontinuation\n").is_empty());
    assert_eq!(
        sanitize::from_record(tail.as_slice()).len(),
        tail.len() - 37
    );
}
// The DLL transcodes with MultiByteToWideChar/WideCharToMultiByte (logs.rs,
// run by check_logs.py under Wine); here, the letters these tests use.
fn test_transcode(code_page: u32, line: &mut [u8], len: usize) -> Result<usize, u32> {
    let mut text = String::new();
    for &b in &line[..len] {
        let c = match (code_page, b) {
            (_, 0..=0x7f) => b as u32,
            (1251, 0xc0..=0xff) => 0x410 + (b as u32 - 0xc0),
            (1251, 0xa8) => 0x401,
            (1251, 0xb8) => 0x451,
            (1252 | 28591, 0xa0..=0xff) => b as u32,
            (1250, _) => match b {
                0xb3 => 0x142, // ł
                0x9c => 0x15b, // ś
                0xf3 => 0xf3,  // ó
                0xea => 0x119, // ę
                0xb9 => 0x105, // ą
                _ => return Err(sanitize::ERROR_UNSAFE_TEXT),
            },
            _ => return Err(sanitize::ERROR_UNSAFE_TEXT),
        };
        text.push(char::from_u32(c).unwrap());
    }
    if text.len() > line.len() {
        return Err(sanitize::ERROR_LONG_LINE);
    }
    line[..text.len()].copy_from_slice(text.as_bytes());
    Ok(text.len())
}
fn legacy(bytes: &[u8], step: usize, code_page: u32) -> Result<(Vec<u8>, u64, u64), u32> {
    let mut input = Input { bytes, step };
    let mut filter = sanitize::Filter::legacy(&mut input, Some((code_page, test_transcode)));
    let mut output = Vec::new();
    let mut buf = [0; 73];
    loop {
        let n = filter.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.extend_from_slice(&buf[..n]);
    }
    assert_eq!(output.len() as u64, filter.emitted);
    Ok((output, filter.removed_lines, filter.transcoded_lines))
}
fn encode(text: &str, code_page: u32) -> Vec<u8> {
    text.chars()
        .map(|c| match (code_page, c as u32) {
            (_, 0..=0x7f) => c as u8,
            (1251, 0x410..=0x44f) => (c as u32 - 0x410 + 0xc0) as u8,
            (1252, 0xa0..=0xff) => c as u8,
            (1250, 0x142) => 0xb3,
            (1250, 0x15b) => 0x9c,
            (1250, 0xf3) => 0xf3,
            _ => panic!("{c}"),
        })
        .collect()
}
#[test]
fn legacy_code_page_lines_are_transcoded_to_utf8() {
    // Kobalt's log (2026-10-26, -localization=russian): the audio devices in cp1251.
    for (code_page, device) in [
        (1251, "Динамики (Realtek(R) Audio)"),
        (1250, "Głośniki (Realtek(R) Audio)"),
        (1252, "Kopfhörer (Realtek(R) Audio)"),
    ] {
        let text = format!(
            "{SAFE}sound <info>\t[11:00:25.100]\taudio devices count: 2\r\nsound <info>\t[11:00:25.100]\t0: {device}\r\nsound <info>\t[11:00:25.100]\trole: NotDefaultDevice \r\nsound <info>\t[11:00:25.100]\t1: {device}\r\n{SAFE}"
        );
        for step in [1, 7, 8192] {
            let (out, removed, transcoded) =
                legacy(&encode(&text, code_page), step, code_page).unwrap();
            assert_eq!(String::from_utf8(out).unwrap(), text);
            assert_eq!((removed, transcoded), (0, 2));
        }
        // UTF-8 lines pass unchanged and are not counted.
        assert_eq!(
            legacy(text.as_bytes(), 5, code_page).unwrap(),
            (text.as_bytes().to_vec(), 0, 0)
        );
        // Without a code page (the crash details' tail) the source still fails.
        assert_eq!(
            filter(&encode(&text, code_page), 5),
            Err(sanitize::ERROR_UNSAFE_TEXT)
        );
    }
}
#[test]
fn legacy_lines_still_fail_on_control_bytes_utf16_and_length_and_stay_filtered() {
    let mut control = encode("0: Динамики", 1251);
    control.insert(3, 0x1b);
    for text in [
        control.as_slice(),
        b"\xc4\xe8\0\xed\xe0\n",
        &"Динамики"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
        b"\x98 not cp1251\n",
    ] {
        assert_eq!(legacy(text, 3, 1251), Err(sanitize::ERROR_UNSAFE_TEXT));
    }
    // The 64 KiB rule is on the UTF-8: 40000 Cyrillic bytes are 80000 bytes.
    assert_eq!(
        legacy(&vec![0xc4; 40000], 8192, 1251),
        Err(sanitize::ERROR_LONG_LINE)
    );
    // A sensitive record in cp1251 is removed, with its continuation, as in UTF-8.
    let text = format!(
        "{SAFE}core <info>\t[01:02:03.456]\tИгрок password=SYNTHETIC Динамики\r\nпродолжение-SYNTHETIC\r\n{SAFE}"
    );
    for step in [1, 2, 8192] {
        let (out, removed, transcoded) = legacy(&encode(&text, 1251), step, 1251).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(!out.contains("SYNTHETIC"), "{out}");
        assert_eq!(out.matches("managed arena").count(), 2);
        assert_eq!((removed, transcoded), (2, 2));
    }
}
