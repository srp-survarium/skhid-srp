#[path = "../src/crash_text.rs"]
mod crash_text;
#[path = "../src/details.rs"]
mod details;
#[path = "../src/notice_text.rs"]
mod notice_text;

use notice_text::Language::*;

#[test]
fn every_report_state_has_localized_instructions_and_fits_its_buffers() {
    for language in [English, Russian, Polish, Ukrainian, German] {
        let t = crash_text::text(language);
        for s in [t.instructions, t.no_file, t.manual] {
            assert!(s.contains("#bugreports"), "{language:?}");
        }
        assert!(t.instructions.contains(t.discord));
        assert!(t.no_file.contains(t.discord));
        assert!(t.instructions.contains(t.copy) && t.no_file.contains(t.copy));
        assert!(t.blocked.contains("#bugreports") && t.blocked.contains("Windows"));
        assert!(t.copy.encode_utf16().count() < 64 && t.copied.encode_utf16().count() < 64);
        assert!(
            t.browser_failed.encode_utf16().count() + t.manual.encode_utf16().count() + 4 < 512
        );
        assert!(t.manual.contains("discord.gg/QUmJg3gBhg") && t.manual.contains("#bugreports"));
        assert!(!t.manual.contains(t.discord));
        assert!(t.title.encode_utf16().count() < 128);
        assert!(t.discord.encode_utf16().count() < 64);
        for s in [
            t.intro, t.saved, t.partial, t.failed, t.blocked, t.error, t.file, t.folder, t.close,
            t.copy, t.copied,
        ] {
            assert!(!s.is_empty(), "{language:?}");
        }
    }
}

fn text(f: impl FnOnce(&mut details::Buffer<512>) -> core::fmt::Result) -> String {
    let mut b = details::Buffer::<512>::new();
    f(&mut b).unwrap();
    b.as_str().to_string()
}

#[test]
fn exception_codes_have_their_names_and_what_was_touched() {
    let e = |code, params: &[usize], cpp| text(|w| details::write_exception(w, code, params, cpp));
    assert_eq!(
        e(0xC000_0005, &[1, 0], None),
        "0xC0000005 ACCESS_VIOLATION (write to 0x00000000)"
    );
    assert_eq!(
        e(0xC000_0005, &[0, 0x1C], None),
        "0xC0000005 ACCESS_VIOLATION (read of 0x0000001C)"
    );
    assert_eq!(
        e(0xC000_0005, &[8, 0x0BAD_F00D], None),
        "0xC0000005 ACCESS_VIOLATION (execution of 0x0BADF00D)"
    );
    assert_eq!(e(0xC000_0005, &[], None), "0xC0000005 ACCESS_VIOLATION");
    assert_eq!(
        e(0xC000_0006, &[0, 0x1000, 0xC000_009C], None),
        "0xC0000006 IN_PAGE_ERROR (read of 0x00001000, status 0xC000009C)"
    );
    assert_eq!(e(0xC000_00FD, &[], None), "0xC00000FD STACK_OVERFLOW");
    assert_eq!(e(0xC000_0094, &[], None), "0xC0000094 INT_DIVIDE_BY_ZERO");
    assert_eq!(
        e(0xC000_0409, &[2], None),
        "0xC0000409 STACK_BUFFER_OVERRUN"
    );
    assert_eq!(
        e(
            0xE06D_7363,
            &[0x1993_0520, 0, 0],
            Some(".?AVbad_alloc@std@@")
        ),
        "0xE06D7363 C++ exception (.?AVbad_alloc@std@@)"
    );
    assert_eq!(e(0x1234_5678, &[], None), "0x12345678 unknown");
}

#[test]
fn addresses_are_module_plus_offset() {
    let at = |address, module| text(|w| details::write_location(w, address, module));
    assert_eq!(
        at(0x002E_1E20, Some(("survarium_skhid.exe", 0x10000))),
        "survarium_skhid.exe+0x2D1E20"
    );
    assert_eq!(
        at(0x7701_0005, Some(("ntdll.dll", 0x7701_0000))),
        "ntdll.dll+0x5"
    );
    assert_eq!(at(0x0BAD_F00D, None), "0x0BADF00D (not in a module)");
    let code = |c| text(|w| details::write_code(w, c));
    assert_eq!(code(0x8007_001F), "0x8007001F");
    assert_eq!(code(5), "5");
    assert_eq!(code(0xC000_0022), "0xC0000022");
}

#[test]
fn a_refused_capture_is_told_from_a_folder_problem() {
    use details::Failure::*;
    // MiniDumpWriteDump's HRESULTs: the game's memory could not be read.
    for code in [0x8007_001F, 0x8007_012B, 0x8007_0005, 0x8007_0057] {
        assert_eq!(details::failure(true, code), Capture, "{code:#x}");
    }
    // Making the file: folder or permission; and a full disk while writing.
    for code in [5, 3, 32, 206] {
        assert_eq!(details::failure(false, code), File, "{code}");
    }
    for code in [0x8007_0070, 0x8007_0027, 112] {
        assert_eq!(details::failure(true, code), File, "{code:#x}");
    }
    for language in [English, Russian, Polish, Ukrainian, German] {
        let t = crash_text::text(language);
        assert_eq!(crash_text::status(t, true, false, File), t.saved);
        assert_eq!(crash_text::status(t, false, true, Capture), t.partial);
        assert_eq!(crash_text::status(t, false, false, File), t.failed);
        assert_eq!(crash_text::status(t, false, false, Capture), t.blocked);
    }
    assert_eq!(
        crash_text::text(English).blocked,
        "An antivirus or Windows security feature blocked the crash dump. The crash details below are still useful: copy them to #bugreports."
    );
}

#[test]
fn the_stack_scan_takes_addresses_right_after_a_call() {
    let ends = |tail: &[u8]| {
        let mut before = [0x90u8; 8];
        before[8 - tail.len()..].copy_from_slice(tail);
        details::follows_call(&before)
    };
    assert!(ends(&[0xE8, 0x10, 0x20, 0x30, 0x40])); // call rel32
    assert!(ends(&[0xFF, 0xD0])); // call eax
    assert!(ends(&[0xFF, 0x10])); // call [eax]
    assert!(ends(&[0xFF, 0x55, 0x08])); // call [ebp+8]
    assert!(ends(&[0xFF, 0x14, 0x24])); // call [esp]
    assert!(ends(&[0xFF, 0x54, 0x24, 0x08])); // call [esp+8]
    assert!(ends(&[0xFF, 0x15, 1, 2, 3, 4])); // call [disp32]
    assert!(ends(&[0xFF, 0x90, 1, 2, 3, 4])); // call [eax+disp32]
    assert!(ends(&[0xFF, 0x14, 0x85, 1, 2, 3, 4])); // call [eax*4+disp32]
    assert!(!ends(&[]));
    assert!(!ends(&[0xFF, 0xE0])); // jmp eax
    assert!(!ends(&[0x8B, 0x45, 0x08])); // mov eax, [ebp+8]
}

#[test]
fn a_folder_with_non_ascii_letters_is_named_in_short() {
    let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    assert!(!details::needs_simple_path(&wide(
        "C:\\Games\\Survarium\\game\\binaries\\x86\\"
    )));
    assert!(details::needs_simple_path(&wide(
        "C:\\Users\\Иван\\AppData\\Local\\Survarium\\"
    )));
    assert!(details::needs_simple_path(&wide("D:\\Gry\\Żółw\\")));
    let short = |s: &str, max| text(|w| details::write_short_path(w, &wide(s), max));
    assert_eq!(short("C:\\Games\\", 60), "C:\\Games\\");
    let long = "C:\\Users\\Иван\\AppData\\Local\\Survarium\\game\\binaries\\x86\\";
    let cut = short(long, 30);
    assert_eq!(cut.chars().count(), 30);
    assert!(
        cut.starts_with("C:\\…") && cut.ends_with("\\game\\binaries\\x86\\"),
        "{cut}"
    );
    for language in [English, Russian, Polish, Ukrainian, German] {
        let [before, after] = crash_text::text(language).simple_path;
        assert!(
            before.ends_with('(') && after.starts_with(')'),
            "{language:?}"
        );
        assert!(after.contains("C:\\Games\\Survarium"), "{language:?}");
    }
}

#[test]
fn the_log_tail_is_its_last_whole_lines() {
    let log = b"cut line\r\none\r\ntwo\r\nthree\r\n";
    assert_eq!(details::last_lines(log, 2, false), b"two\r\nthree\r\n");
    assert_eq!(
        details::last_lines(log, 9, false),
        b"one\r\ntwo\r\nthree\r\n"
    );
    assert_eq!(details::last_lines(log, 9, true), &log[..]);
    assert_eq!(details::last_lines(b"a\nb", 1, true), b"b");
    assert_eq!(details::last_lines(b"no newline", 5, false), b"");
    assert_eq!(details::last_lines(log, 0, true), b"");
    // A bounded buffer keeps whole characters only.
    use core::fmt::Write;
    let mut b = details::Buffer::<5>::new();
    assert!(b.write_str("abcжд").is_err());
    assert_eq!(b.as_str(), "abcж");
}
