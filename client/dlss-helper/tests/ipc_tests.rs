//! The pipe's records, on the build host:
//!   rustc --edition 2024 --test tests/ipc_tests.rs -o target/ipc-tests && target/ipc-tests
#![allow(dead_code)]
#[path = "../src/ipc.rs"]
mod ipc;

use ipc::*;

fn hello() -> HelloMsg {
    HelloMsg { magic: MAGIC, version: VERSION, device_uuid: [7; 16], device_luid: [0; 8], out_w: 1920, out_h: 1080, quality: 2, helper_mode: 0 }
}

#[test]
fn a_hello_is_read_as_the_game_wrote_it() {
    let mut bytes = Vec::new();
    hello().write_to(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 48);
    assert_eq!(&bytes[..4], b"DLSS");
    let h = Hello::try_from(HelloMsg::read_from(&mut bytes.as_slice()).unwrap()).unwrap();
    assert_eq!((h.out, h.quality, h.mode, h.device_uuid), ((1920, 1080), Quality::Quality, Mode::Dlss, [7; 16]));
}

#[test]
fn a_hello_of_another_protocol_or_of_unknown_values_is_refused() {
    for change in [
        (|m: &mut HelloMsg| m.magic = 1) as fn(&mut HelloMsg),
        |m| m.version = VERSION + 1,
        |m| m.quality = 4,
        |m| m.helper_mode = 2,
        |m| m.out_w = 0,
        |m| m.out_h = 20000,
    ] {
        let mut m = hello();
        change(&mut m);
        assert!(Hello::try_from(m).is_err());
    }
}

#[test]
fn a_short_read_is_the_pipe_closed() {
    let bytes = [0u8; 10];
    let e = HelloMsg::read_from(&mut bytes.as_slice()).err().unwrap();
    assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof);
}

fn frame(quit: u32, wait: u64, signal: u64) -> FrameMsg {
    FrameMsg { frame: 9, wait_value: wait, signal_value: signal, jitter_x: 0.25, jitter_y: -0.5, mv_scale_x: 1.0, mv_scale_y: 1.0, reset: 1, quit }
}

#[test]
fn the_commands_between_frames() {
    assert!(matches!(Command::try_from(frame(1, 0, 0)), Ok(Command::Quit)));
    assert!(matches!(Command::try_from(frame(2, 0, 0)), Ok(Command::Resize)));
    match Command::try_from(frame(0, 19, 20)) {
        Ok(Command::Frame(f)) => assert_eq!((f.number, f.wait, f.signal, f.jitter, f.reset), (9, 19, 20, (0.25, -0.5), true)),
        other => panic!("{other:?}"),
    }
    // a signal value not above the wait value would never be signalled in order
    assert!(Command::try_from(frame(0, 20, 20)).is_err());
    assert!(Command::try_from(frame(3, 19, 20)).is_err());
}

#[test]
fn a_setup_needs_its_handles_and_one_render_size() {
    let img = |w, h| SharedImageMsg { handle: 0x44, alloc_size: 1 << 20, memory_type: 1, format: 37, width: w, height: h, usage: 6, pad: 0 };
    let good = SetupMsg { img: [img(1280, 720), img(1280, 720), img(1280, 720), img(1920, 1080)], semaphore: 0x48, dlss_flags: 2, pad: 0 };
    let s = Setup::try_from(good).unwrap();
    assert_eq!((s.img[3].size, s.semaphore, s.dlss_flags), ((1920, 1080), 0x48, 2));
    let mut m = good;
    m.img[1] = img(1280, 721);
    assert!(Setup::try_from(m).is_err());
    let mut m = good;
    m.img[2].handle = 0;
    assert!(Setup::try_from(m).is_err());
    let mut m = good;
    m.img[0].memory_type = 32;
    assert!(Setup::try_from(m).is_err());
    let mut m = good;
    m.semaphore = 0;
    assert!(Setup::try_from(m).is_err());
}

#[test]
fn a_reply_text_is_cut_on_a_character_and_ends_with_nul() {
    let long = "ж".repeat(200); // 400 bytes
    let t = text(&long);
    assert_eq!(t[255], 0);
    let end = t.iter().position(|&b| b == 0).unwrap();
    assert_eq!(end, 254);
    assert!(std::str::from_utf8(&t[..end]).is_ok());
    assert_eq!(&text("no")[..3], b"no\0");
}
