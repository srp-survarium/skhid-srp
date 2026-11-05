#[path = "../src/breadcrumbs.rs"]
mod breadcrumbs;
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

#[test]
fn the_local_offset_is_signed_hours_and_minutes() {
    let offset = |m| text(|w| details::write_offset(w, m));
    assert_eq!(offset(180), " (local +03:00)");
    assert_eq!(offset(0), " (local +00:00)");
    assert_eq!(offset(-210), " (local -03:30)");
    assert_eq!(offset(345), " (local +05:45)");
}

/// A ring as vostok/breadcrumbs.rs lays it out, at `at`, its counter at
/// 1000 ticks a second.
fn ring_bytes(at: u32, entries: &[(u32, u8, u8, u16, u64, [u32; 4])], next: u32) -> Vec<u8> {
    let mut b = vec![0u8; breadcrumbs::BYTES];
    let put = |b: &mut Vec<u8>, o: usize, v: &[u8]| b[o..o + v.len()].copy_from_slice(v);
    put(&mut b, 0, &at.to_le_bytes());
    put(&mut b, 4, &(at ^ breadcrumbs::CHECK).to_le_bytes());
    put(&mut b, 8, &1u16.to_le_bytes());
    put(&mut b, 10, &32u16.to_le_bytes());
    put(&mut b, 12, &1024u32.to_le_bytes());
    put(&mut b, 16, &next.to_le_bytes());
    put(&mut b, 24, &1000u64.to_le_bytes());
    // The match: 0x4d (with the top bit as the options carry it), map 12, 8 places, level 5, Blackout; local place 3.
    put(&mut b, 32, &(77u32 | 1 << 31).to_le_bytes());
    put(&mut b, 36, &[12, 8]);
    put(&mut b, 38, &5u16.to_le_bytes());
    put(&mut b, 40, &(1u32 << 31 | 1).to_le_bytes());
    put(&mut b, 44, &4u32.to_le_bytes());
    // Reading place 3's player state from byte 420, at 9800.
    put(&mut b, 48, &(2u32 | 3 << 8).to_le_bytes());
    put(&mut b, 52, &420u32.to_le_bytes());
    put(&mut b, 56, &9800u64.to_le_bytes());
    for &(seq, kind, a, x, counter, values) in entries {
        let o = 64 + 32 * ((seq as usize - 1) % 1024);
        put(&mut b, o, &seq.to_le_bytes());
        put(&mut b, o + 4, &[kind, a]);
        put(&mut b, o + 6, &x.to_le_bytes());
        put(&mut b, o + 8, &counter.to_le_bytes());
        for (i, v) in values.iter().enumerate() {
            put(&mut b, o + 16 + 4 * i, &v.to_le_bytes());
        }
    }
    b
}

#[test]
fn breadcrumbs_are_the_match_and_the_last_events_before_the_crash() {
    let at = 0x0A4D_D000;
    let bytes = ring_bytes(
        at,
        &[
            (1, 1, 0, 5, 1000, [77, 12, 8, 1]),
            (2, 2, 3, 4, 2500, [4500, 900, 0, 0]),
            (3, 7, 4, 0, 9000, [2, 61000, 0, 0]),
            (4, 8, 2, 0x0103, 9500, [5, 1, 61500, 0]),
            // Being written at the crash: its number not yet its own.
            (5, 99, 0, 0, 9900, [0; 4]),
        ],
        5,
    );
    let mut bytes = bytes;
    bytes[64 + 32 * 4..][..4].copy_from_slice(&0u32.to_le_bytes());
    assert!(breadcrumbs::Ring::parse(&bytes, at + 4).is_none(), "its address is the header's");
    let ring = breadcrumbs::Ring::parse(&bytes, at).unwrap();
    let mut out = details::Buffer::<4096>::new();
    breadcrumbs::write_context(&mut out, &ring).unwrap();
    breadcrumbs::write_reading(&mut out, &ring, 10_000).unwrap();
    breadcrumbs::write_events(&mut out, &ring, 10_000, 200).unwrap();
    assert_eq!(
        out.as_str(),
        "Match:     0x4d, map 12, 8 places, level 5, Blackout; local place 3\r\n\
         Reading:   place 3's player state, from byte 420 of its stream, 0.200 s before the crash\r\n\
         Events:    5 recorded, the last 4 kept: match 1 world 1 death 1 pickup 1\r\n\
         \x20          seconds before the crash:\r\n\
         \x20 -9.000 s  match: 0x4d, map 12, 8 places, level 5, Blackout\r\n\
         \x20 -7.500 s  world: 4500 ms, 900 bytes packed, loaded, local place 3\r\n\
         \x20 -1.000 s  death: place 4, life 2, at 61000 ms\r\n\
         \x20 -0.500 s  pickup: place 3 slot 1 let go: the gun of place 5 kit 1, at 61500 ms\r\n"
    );
}

#[test]
fn a_wrapped_ring_lists_its_last_lines_and_counts_what_it_kept() {
    let at = 0x0A4D_D000;
    let entries: Vec<_> = (1..=1500u32).map(|n| (n, 6, (n % 20) as u8, 0, u64::from(n), [0; 4])).collect();
    let bytes = ring_bytes(at, &entries, 1500);
    let ring = breadcrumbs::Ring::parse(&bytes, at).unwrap();
    assert_eq!(ring.entries().count(), 1024);
    let mut out = details::Buffer::<16384>::new();
    breadcrumbs::write_events(&mut out, &ring, 1500, 3).unwrap();
    assert!(out.as_str().starts_with("Events:    1500 recorded, the last 1024 kept: player-gone 1024\r\n           the last 3, seconds before the crash:\r\n"), "{}", out.as_str());
    assert_eq!(out.as_str().lines().count(), 5);
    assert!(out.as_str().ends_with("  -0.000 s  player gone: place 0\r\n"));
}

#[test]
fn full_dumps_are_asked_for_in_any_case_with_spaces() {
    for value in ["full", "FULL", "Full", "full ", " full\t"] {
        assert!(details::asks_full_dump(value), "{value:?}");
    }
    for value in ["", "small", "fu ll", "1", "fullx"] {
        assert!(!details::asks_full_dump(value), "{value:?}");
    }
}

#[test]
fn a_finished_read_is_no_reading_line() {
    let at = 0x0A4D_D000;
    let mut bytes = ring_bytes(at, &[], 0);
    // Place 3's player state read to byte 600 (patch 78's carry.load).
    bytes[48..52].copy_from_slice(&(3u32 | 3 << 8).to_le_bytes());
    bytes[52..56].copy_from_slice(&600u32.to_le_bytes());
    let ring = breadcrumbs::Ring::parse(&bytes, at).unwrap();
    let mut out = details::Buffer::<256>::new();
    breadcrumbs::write_reading(&mut out, &ring, 10_000).unwrap();
    assert_eq!(out.as_str(), "");
    // A world before its players is one.
    bytes[48..52].copy_from_slice(&1u32.to_le_bytes());
    let ring = breadcrumbs::Ring::parse(&bytes, at).unwrap();
    breadcrumbs::write_reading(&mut out, &ring, 10_000).unwrap();
    assert!(out.as_str().starts_with("Reading:   a world (0xE8), before its players, "), "{}", out.as_str());
}
