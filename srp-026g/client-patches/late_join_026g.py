#!/usr/bin/env python3
"""A late-join client for tests only (patch 11; client_patch.md section 3.11).

    python late_join_026g.py ORIGINAL OUTPUT --test-session N [--server=host:port] [--nasm /path/to/nasm]
        the default build (patch_026g.py's, the late join's Rust and NASM in
        its arena) that passes the login by and connects straight to its
        -server=host:port as the session N (late_join_test_026g.asm), for
        the match server's late_join_native example

The late join itself is part of every default build: the Rust patch
late_join (rust/patches/src/late_join.rs, late_join.toml) and the NASM stubs
of late_join_026g.asm, in the arena (link_026g.py). Before the Rust port
it was late_join_026g.section.bin, built by this tool from NASM and three C
files. Requires NASM (the test stub) and what patch_026g.py's build needs
(the dev shell). Existing executables are never overwritten.
"""
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile

import patch_026g as base

HERE = Path(__file__).resolve().parent
TEST_STUB = HERE / "late_join_test_026g.asm"
# The test session's two sites: the lobby profile's statistics reset is
# jumped over (a direct connection has none), the connect call goes to the
# stub at LATE_JOIN_VA.
RESET = (0x27a1d1, bytes.fromhex("33c0"), bytes.fromhex("eb05"))
CONNECT_VA = 0x1ec0e1
CONNECT = bytes.fromhex("e8fa150000")


def test_stub(nasm, test_session):
    """late_join_test_026g.asm assembled at LATE_JOIN_VA for `test_session`."""
    if not 1 <= test_session <= 0xffffffff:
        raise ValueError("test session must be a nonzero u32")
    with tempfile.TemporaryDirectory(prefix="srp-late-join-") as temporary:
        binary = Path(temporary) / "test.bin"
        subprocess.run([nasm, "-f", "bin", f"-DCAVE={base.LATE_JOIN_VA}", f"-DTEST_SESSION={test_session}",
                        str(TEST_STUB), "-o", str(binary)], check=True)
        return binary.read_bytes()


def build(original, nasm="nasm", test_session=None, server=None):
    """The default client; with `test_session`, the test session's (the stub
    in the room at LATE_JOIN_VA, int3 in the default build)."""
    upload = Path(base.default_bin("section")).read_bytes()
    data, _ = base.build(original, "section", upload, server, True)
    if test_session is None:
        return data
    stub = test_stub(nasm, test_session)
    headers = base.Headers(data)
    _, rva, _, raw, raw_size, _ = next(s for s in headers.sections(data) if s[0] == base.SECTION_NAME)
    at = raw + base.LATE_JOIN_VA - base.IMAGE_BASE - rva
    if set(data[at:at + len(stub)]) != {0xCC} or base.LATE_JOIN_OFFSET + len(stub) > base.WEAPON_PREVIEW_OFFSET:
        raise ValueError("the test stub's room at LATE_JOIN_VA is not int3")
    data[at:at + len(stub)] = stub
    base.patch(data, *RESET)
    base.patch(data, CONNECT_VA, CONNECT,
               b"\xe8" + struct.pack("<i", base.LATE_JOIN_VA - CONNECT_VA - 5))
    if base.neutralize_relocs(data, [(RESET[0], RESET[0] + 2), (CONNECT_VA, CONNECT_VA + 5)]):
        raise ValueError("unexpected relocation in the test-session hooks")
    headers = base.Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, base.pe_checksum(data, headers.checksum_at))
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("original", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--nasm", default="nasm")
    parser.add_argument("--server", help="an embedded default login server, as patch_026g.py --server")
    parser.add_argument("--test-session", type=lambda value: int(value, 0), required=True,
                        help="test only: bypass login and connect directly to -server with this session")
    args = parser.parse_args()
    if args.output.exists() or args.output.resolve() == args.original.resolve():
        parser.error("choose a new output file; existing executables are never overwritten")
    data = build(args.original.read_bytes(), args.nasm, args.test_session,
                 base.parse_server(args.server) if args.server else None)
    with args.output.open("xb") as stream:
        stream.write(data)
    print(f"Late-join test-session client: {args.output}; SHA-256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
