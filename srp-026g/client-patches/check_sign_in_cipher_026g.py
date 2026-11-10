#!/usr/bin/env python3
"""Patch 139 (sign_in_cipher): the sign-in's password goes through srp's own
cipher, where the game's protocol had TLS (docs/notes/sign-in-cipher.md).

    python check_sign_in_cipher_026g.py <original survarium.exe>

Needs Unicorn and Python's `cryptography`: the server here is that library's
X25519, HKDF-SHA-256 and ChaCha20-Poly1305 with the repository's key
(../certs/sign_in_cipher.key), not srp's own code, so the patch's cipher
(crates/sign-in-cipher) is checked against another implementation.

The patched exe's SSL_connect (0x4D4C60), SSL_write (0x4D4D70), SSL_read
(0x4D4CA0) and SSL_get_error (0x4D6E30) run whole, as asio's engine calls
them, on a made-up SSL whose BIOs are this script's two byte queues
(BIO_read 0x44C630, BIO_write 0x44C740 and OpenSSL's generator 0x46B6E0
stubbed; ERR_put_error 0x44A270 recorded), and the begin hook through its
site (0x380504):
1. a sign-in: the hello is queued and more bytes wanted (-1, error 2); the
   server's hello, a byte at a time, connects (1, error 0); the password
   message is one record the server's key opens; the answer is wanted until
   its record is whole, then read as the server sealed it;
2. the game's hello is new each sign-in and a begun sign-in drops what an
   abandoned one left unread;
3. failures fail the call (-1, error 1) with one error queued for
   ERR_get_error: a changed record, a record length no record has, a hello
   of small order, a generator with nothing to give, a call out of turn;
4. every call returns as a cdecl function (its arguments left, EBX ESI EDI
   EBP kept), and the begin site goes on at 0x38050A with ECX and ESP as the
   replaced instructions leave them.
"""
import hashlib
import struct
import sys
from pathlib import Path

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

SSL_CONNECT, SSL_READ, SSL_WRITE, SSL_GET_ERROR = 0x4D4C60, 0x4D4CA0, 0x4D4D70, 0x4D6E30
BIO_READ, BIO_WRITE, RAND_BYTES, ERR_PUT_ERROR = 0x44C630, 0x44C740, 0x46B6E0, 0x44A270
BEGIN, BEGIN_RESUMES = 0x380504, 0x38050A
THREAD_SLOT = 0x870118                           # kernel32 GetCurrentThreadId
NAME = b"skhid sign-in cipher 1"
KEY_FILE = Path(__file__).resolve().parent / ".." / "certs" / "sign_in_cipher.key"
ERROR_NONE, ERROR_SSL, ERROR_WANT_READ = 0, 1, 2
NOT_DONE = 0xFFFFFFFF

DATA = 0x30000000
SSL, RBIO, WBIO, BUFFER, FRAME = (DATA + 0x1000 * i for i in range(5))
STUBS = DATA + 0x10000
THREAD, RETURN = STUBS, STUBS + 0x20
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}


class Server:
    """srp's side, by the `cryptography` library."""

    def __init__(self):
        self.key = X25519PrivateKey.from_private_bytes(bytes.fromhex(KEY_FILE.read_text().strip()))
        self.public = self.key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)

    def accept(self, client_hello):
        fresh = X25519PrivateKey.generate()
        hello = fresh.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        client = X25519PublicKey.from_public_bytes(client_hello)
        secret = self.key.exchange(client) + fresh.exchange(client)
        salt = hashlib.sha256(NAME + self.public + client_hello + hello).digest()
        keys = HKDF(hashes.SHA256(), 64, salt, b"keys").derive(secret)
        self.receive, self.send = ChaCha20Poly1305(keys[:32]), ChaCha20Poly1305(keys[32:])
        self.received = self.sent = 0
        return hello

    @staticmethod
    def nonce(number):
        return bytes(4) + struct.pack("<Q", number)

    def open(self, record):
        header, body = record[:2], record[2:]
        assert struct.unpack("<H", header)[0] == len(body), "the header is the body's length"
        message = self.receive.decrypt(self.nonce(self.received), body, header)
        self.received += 1
        return message

    def seal(self, message):
        header = struct.pack("<H", len(message) + 16)
        body = self.send.encrypt(self.nonce(self.sent), message, header)
        self.sent += 1
        return header + body


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x20000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(SSL + 0xC, RBIO)
        self.word(SSL + 0x10, WBIO)
        for address, stub in ((BIO_READ, self.bio_read), (BIO_WRITE, self.bio_write),
                              (RAND_BYTES, self.rand_bytes), (ERR_PUT_ERROR, self.err_put_error)):
            emu.hook_add(UC_HOOK_CODE, stub, begin=address, end=address)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=BEGIN_RESUMES, end=BEGIN_RESUMES)
        self.from_server, self.to_server, self.errors = bytearray(), bytearray(), []
        self.randoms, self.random_fails = 0, False

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def returns(self, result):
        """A stubbed cdecl function's return."""
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def arguments(self, count):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        return [self.read(sp + 4 + 4 * i) for i in range(count)]

    def bio_read(self, emu, address, size, _):
        bio, room, length = self.arguments(3)
        assert bio == RBIO, "reads the SSL's rbio"
        taken = bytes(self.from_server[:length])
        del self.from_server[:length]
        emu.mem_write(room, taken)
        self.returns(len(taken) if taken else -1)                # an empty pair: -1 (and its retry flag)

    def bio_write(self, emu, address, size, _):
        bio, data, length = self.arguments(3)
        assert bio == WBIO, "writes the SSL's wbio"
        self.to_server += emu.mem_read(data, length)
        self.returns(length)

    def rand_bytes(self, emu, address, size, _):
        room, length = self.arguments(2)
        assert length == 32
        self.randoms += 1
        emu.mem_write(room, hashlib.sha256(b"random %d" % self.randoms).digest())
        self.returns(0 if self.random_fails else 1)

    def err_put_error(self, emu, address, size, _):
        library, function, reason, file, line = self.arguments(5)
        assert library == 20 and reason != 0, "an SSL error with a reason"
        assert bytes(emu.mem_read(file, 20)) == b"skhid sign-in cipher"
        self.errors.append(reason)
        self.returns(0)

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def call(self, function, *args):
        """One cdecl call to its return: EAX."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        for reg in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX):
            self.emu.reg_write(reg, 0xA0A0A0A0)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(function, 0, count=200000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN, hex(self.emu.reg_read(UC_X86_REG_EIP))
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4, "cdecl: the arguments stay"
        for reg, value in KEPT.items():
            assert self.emu.reg_read(reg) == value, hex(reg)
        return self.emu.reg_read(UC_X86_REG_EAX)

    def asked(self, function, *args):
        """A call as asio's engine makes it: its result, then SSL_get_error's."""
        result = self.call(function, SSL, *args)
        return result, self.call(SSL_GET_ERROR, SSL, result)

    def begin(self):
        """on_user_name_answer_received at the taken user name, to the handshake's start."""
        sp = STACK + 0x8000
        self.word(FRAME + 8, 0x12345678)
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_EBP, FRAME)
        self.emu.reg_write(UC_X86_REG_ECX, 0)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(BEGIN, 0, count=2000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == BEGIN_RESUMES
        assert self.emu.reg_read(UC_X86_REG_ECX) == 0x12345678, "mov ecx,[ebp+8] replayed"
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp - 0x30, "sub esp,30h replayed"
        assert self.emu.reg_read(UC_X86_REG_EBP) == FRAME

    def connect(self, server):
        """A sign-in through the hellos."""
        self.begin()
        self.to_server.clear()
        self.errors.clear()
        assert self.asked(SSL_CONNECT) == (NOT_DONE, ERROR_WANT_READ)
        hello = bytes(self.to_server)
        assert len(hello) == 32, "the hello: one X25519 public key"
        self.to_server.clear()
        # asio sends it and calls again: nothing of the server is here yet.
        assert self.asked(SSL_CONNECT) == (NOT_DONE, ERROR_WANT_READ)
        answer = server.accept(hello)
        for byte in answer[:-1]:
            self.from_server.append(byte)
            assert self.asked(SSL_CONNECT) == (NOT_DONE, ERROR_WANT_READ)
        self.from_server.append(answer[-1])
        assert self.asked(SSL_CONNECT) == (1, ERROR_NONE)
        assert not self.to_server and not self.from_server and not self.errors
        return hello

    def write(self, message):
        self.emu.mem_write(BUFFER, message)
        return self.asked(SSL_WRITE, BUFFER, len(message))

    def read_into(self, room):
        self.emu.mem_write(BUFFER, b"\xEE" * (room + 8))
        result, error = self.asked(SSL_READ, BUFFER, room)
        return result, error, bytes(self.emu.mem_read(BUFFER, room + 8))

    def fails(self, function, *args):
        """A call that fails, with exactly one error queued."""
        self.errors.clear()
        assert self.asked(function, *args) == (NOT_DONE, ERROR_SSL)
        assert len(self.errors) == 1, self.errors
        # Nothing goes on after it: the sign-in is over.
        assert self.asked(SSL_READ, BUFFER, 0x52) == (NOT_DONE, ERROR_SSL)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    run, server = Run(bytes(built)), Server()

    first_hello = run.connect(server)
    password = b"\x03" + b"0123456789abcdef0123456789abcdef01234567" + b"\x0d" + b"test-password"
    assert run.write(password) == (len(password), ERROR_NONE)
    record = bytes(run.to_server)
    assert len(record) == 2 + len(password) + 16 and password[1:] not in record
    assert server.open(record) == password
    run.to_server.clear()
    assert run.read_into(0x52)[:2] == (NOT_DONE, ERROR_WANT_READ)
    answer = b"\x08" + struct.pack("<IIH", 0xCAFE0001, 0x7F000001, 1235)
    sealed = server.seal(answer)
    for byte in sealed[:-1]:
        run.from_server.append(byte)
        assert run.read_into(0x52)[:2] == (NOT_DONE, ERROR_WANT_READ)
    run.from_server.append(sealed[-1])
    result, error, room = run.read_into(0x52)
    assert (result, error) == (len(answer), ERROR_NONE)
    assert room == answer + b"\xEE" * (0x52 + 8 - len(answer)), "the answer, and nothing past it"
    print("ok a sign-in: the hello, the password's record opened by the server's key, the answer read")

    # A second answer in a room too small for it comes in parts.
    run.from_server += server.seal(b"0123456789")
    assert run.read_into(4) == (4, ERROR_NONE, b"0123" + b"\xEE" * 8)
    assert run.read_into(8)[:2] == (6, ERROR_NONE)
    assert bytes(run.emu.mem_read(BUFFER, 6)) == b"456789"
    print("ok a message longer than the room is read in parts, never past the room")

    # A sign-in abandoned after its hello; the next starts over.
    run.begin()
    assert run.asked(SSL_CONNECT) == (NOT_DONE, ERROR_WANT_READ)
    run.from_server += b"left unread"
    second_hello = run.connect(server)
    assert second_hello != first_hello and len({first_hello, second_hello, bytes(32)}) == 3
    assert run.write(b"\x02digest") == (7, ERROR_NONE)
    assert server.open(bytes(run.to_server)) == b"\x02digest"
    print("ok a new sign-in: a new hello, nothing of the abandoned one read")

    # Failures.
    run.connect(server)
    record = bytearray(server.seal(answer))
    record[7] ^= 1
    run.from_server += record
    run.fails(SSL_READ, BUFFER, 0x52)
    run.from_server.clear()

    run.connect(server)
    run.from_server += struct.pack("<H", 15)
    run.fails(SSL_READ, BUFFER, 0x52)
    run.from_server.clear()

    run.begin()
    assert run.asked(SSL_CONNECT) == (NOT_DONE, ERROR_WANT_READ)
    run.from_server += bytes(32)
    run.fails(SSL_CONNECT)

    run.begin()
    run.random_fails = True
    run.fails(SSL_CONNECT)
    run.random_fails = False

    # No sign-in begun (the last failed): each call fails.
    run.fails(SSL_CONNECT)
    run.fails(SSL_WRITE, BUFFER, 4)
    run.begin()
    run.fails(SSL_WRITE, BUFFER, 4)
    print("ok a changed record, a bad length, a hello of small order, no random bytes and calls out of turn "
          "fail with one queued error each")

    run.connect(server)
    assert run.write(password) == (len(password), ERROR_NONE) and server.open(bytes(run.to_server)) == password
    print("ok a sign-in after the failures works; every call returned as cdecl, EBX ESI EDI EBP kept")


if __name__ == "__main__":
    main()
