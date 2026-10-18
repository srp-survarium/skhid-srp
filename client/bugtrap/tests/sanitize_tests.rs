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
