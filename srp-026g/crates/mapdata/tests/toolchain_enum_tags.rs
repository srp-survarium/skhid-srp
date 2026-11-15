//! The toolchain's fix of rust-lang/rust#159035 (`flake.nix` pins nightly-2026-07-12): LLVM combined loads of an enum's tag
//! across a `None` whose tag is -1 into a read at an offset of 2^32 - 1, out of bounds (a segfault when unmapped, a wrong value
//! when mapped). The nightly of 2026-05-29 before it segfaulted a bot test (`goals.rs`, a_wooden_crate_is_no_cover). This is
//! rust-lang/rust's own regression test, `tests/ui/codegen/dont-miscompile-enums.rs` (MIT or Apache-2.0, as Rust), as a test of
//! ours: it crashes or answers wrong when built with the old toolchain in release, as the suite builds it.

enum Inner {
    A(u32),
    B(u32),
}

struct Big {
    _pad: u64,
    inner: Inner,
}

struct Small {
    a: u16,
    b: u16,
    _f: fn(),
}

enum Checksum {
    X(Big),
    Y(Small),
}

impl Checksum {
    fn finalize(self) -> u32 {
        match self {
            Checksum::X(h) => match h.inner {
                Inner::A(s) => s,
                Inner::B(s) => s,
            },
            Checksum::Y(s) => (u32::from(s.b) << 16) | u32::from(s.a),
        }
    }
}

#[inline(never)]
fn run(c: Option<Checksum>) -> Option<u32> {
    c.map(|c| c.finalize())
}

fn nothing() {}

#[test]
fn an_option_of_a_two_variant_enum_is_read_by_its_own_tag() {
    assert_eq!(run(std::hint::black_box(None)), None);
    let big = Checksum::X(Big { _pad: 7, inner: Inner::B(0x1234_5678) });
    assert_eq!(run(std::hint::black_box(Some(big))), Some(0x1234_5678));
    let small = Checksum::Y(Small { a: 0x0102, b: 0x0304, _f: nothing });
    assert_eq!(run(std::hint::black_box(Some(small))), Some(0x0304_0102));
}
