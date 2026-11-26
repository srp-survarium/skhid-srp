//! CRYPT32 / ADVAPI32 subset for the resource signature check, with real answers:
//!
//! * `vostok::crypt::validate_certificate_and_get_public_key` 0x20072E020 takes the public key of a known certificate
//!   in the exe's own Authenticode PKCS#7 (an expired certificate is accepted): CryptQueryObject,
//!   CertFindCertificateInStore, the chain check, CryptImportPublicKeyInfo, CryptBinaryToStringA.
//! * `verify_sign_with_crypt_handles` 0x20072E7F0 checks each volume's FAT: CryptCreateHash(SHA-1),
//!   CryptHashData, CryptVerifySignatureA. Implemented for real: RSA PKCS#1 v1.5 with the certificate's
//!   key (CryptoAPI signatures are little-endian).
#![allow(non_snake_case)]

use crate::hlog;
use crate::mem::heap_alloc;
use std::collections::HashMap;
use std::sync::Mutex;

type A = u64;

// ------------------------------------------------------------------ DER

fn tlv(b: &[u8], i: usize) -> Option<(u8, usize, usize)> {
    let tag = *b.get(i)?;
    let mut l = *b.get(i + 1)? as usize;
    let mut j = i + 2;
    if l & 0x80 != 0 {
        let n = l & 0x7F;
        l = 0;
        for k in 0..n {
            l = (l << 8) | *b.get(j + k)? as usize;
        }
        j += n;
    }
    Some((tag, j, l))
}

fn children(b: &[u8], start: usize, len: usize) -> Vec<(u8, usize, usize, usize)> {
    let mut v = Vec::new();
    let mut i = start;
    while i < start + len {
        let Some((t, c, l)) = tlv(b, i) else { break };
        v.push((t, i, c, l));
        i = c + l;
    }
    v
}

fn oid_str(b: &[u8]) -> String {
    let mut a = vec![(b[0] / 40) as u64, (b[0] % 40) as u64];
    let mut v = 0u64;
    for &x in &b[1..] {
        v = (v << 7) | (x & 0x7F) as u64;
        if x & 0x80 == 0 {
            a.push(v);
            v = 0;
        }
    }
    a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(".")
}

fn der_time_to_filetime(b: &[u8], tag: u8) -> u64 {
    // UTCTime YYMMDDHHMMSSZ / GeneralizedTime YYYYMMDDHHMMSSZ
    let s = String::from_utf8_lossy(b);
    let (y, rest) = if tag == 0x17 {
        let yy: i64 = s[0..2].parse().unwrap_or(0);
        (if yy < 50 { 2000 + yy } else { 1900 + yy }, &s[2..])
    } else {
        (s[0..4].parse().unwrap_or(1970), &s[4..])
    };
    let n = |a: usize| -> i64 { rest[a..a + 2].parse().unwrap_or(0) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = (y - 1900) as i32;
    tm.tm_mon = (n(0) - 1) as i32;
    tm.tm_mday = n(2) as i32;
    tm.tm_hour = n(4) as i32;
    tm.tm_min = n(6) as i32;
    tm.tm_sec = n(8) as i32;
    let t = unsafe { libc::timegm(&mut tm) };
    crate::files::unix_to_filetime(t, 0)
}

struct Cert {
    der: Vec<u8>,
    serial_le: Vec<u8>,
    sig_alg: String,
    issuer: Vec<u8>,
    subject: Vec<u8>,
    not_before: u64,
    not_after: u64,
    spki_alg: String,
    spki_params: Vec<u8>,
    pubkey: Vec<u8>,
}

fn parse_pkcs7_certs(p7: &[u8]) -> Vec<Cert> {
    let mut out = Vec::new();
    let Some((_, c, l)) = tlv(p7, 0) else { return out };
    let ci = children(p7, c, l);
    if ci.len() < 2 {
        return out;
    }
    let Some((_, c0, l0)) = tlv(p7, ci[1].2) else { return out };
    let sd = children(p7, c0, l0);
    let Some(certs) = sd.iter().find(|x| x.0 == 0xA0) else { return out };
    for (_, i, c, l) in children(p7, certs.2, certs.3) {
        let tbs = children(p7, c, l)[0];
        let f = children(p7, tbs.2, tbs.3);
        let k = if f[0].0 == 0xA0 { 1 } else { 0 };
        let serial = &p7[f[k].2..f[k].2 + f[k].3];
        let sigalg = children(p7, f[k + 1].2, f[k + 1].3);
        let validity = children(p7, f[k + 3].2, f[k + 3].3);
        let spki = children(p7, f[k + 5].2, f[k + 5].3);
        let alg = children(p7, spki[0].2, spki[0].3);
        let bits = &p7[spki[1].2..spki[1].2 + spki[1].3];
        out.push(Cert {
            der: p7[i..c + l].to_vec(),
            serial_le: serial.iter().rev().copied().collect(),
            sig_alg: oid_str(&p7[sigalg[0].2..sigalg[0].2 + sigalg[0].3]),
            issuer: p7[f[k + 2].1..f[k + 2].2 + f[k + 2].3].to_vec(),
            subject: p7[f[k + 4].1..f[k + 4].2 + f[k + 4].3].to_vec(),
            not_before: der_time_to_filetime(&p7[validity[0].2..validity[0].2 + validity[0].3], validity[0].0),
            not_after: der_time_to_filetime(&p7[validity[1].2..validity[1].2 + validity[1].3], validity[1].0),
            spki_alg: oid_str(&p7[alg[0].2..alg[0].2 + alg[0].3]),
            spki_params: alg.get(1).map(|a| p7[a.1..a.2 + a.3].to_vec()).unwrap_or_default(),
            pubkey: bits[1..].to_vec(),
        });
    }
    out
}

// ------------------------------------------------------------------ objects

fn low_copy(b: &[u8]) -> u64 {
    let p = unsafe { heap_alloc(b.len().max(1), true) };
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), p as *mut u8, b.len()) };
    p as u64
}
fn low_cstr(s: &str) -> u64 {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    low_copy(&v)
}

/// Build CERT_INFO + CERT_CONTEXT (x64 layouts) in exe-visible memory.
fn make_cert_context(c: &Cert, store: u64) -> u64 {
    unsafe {
        let info = heap_alloc(0x100, true) as u64;
        let w32 = |a: u64, v: u32| (a as *mut u32).write_unaligned(v);
        let w64 = |a: u64, v: u64| (a as *mut u64).write_unaligned(v);
        w32(info, 2); // v3
        w32(info + 0x08, c.serial_le.len() as u32);
        w64(info + 0x10, low_copy(&c.serial_le));
        w64(info + 0x18, low_cstr(&c.sig_alg));
        w32(info + 0x30, c.issuer.len() as u32);
        w64(info + 0x38, low_copy(&c.issuer));
        w64(info + 0x40, c.not_before);
        w64(info + 0x48, c.not_after);
        w32(info + 0x50, c.subject.len() as u32);
        w64(info + 0x58, low_copy(&c.subject));
        w64(info + 0x60, low_cstr(&c.spki_alg));
        w32(info + 0x68, c.spki_params.len() as u32);
        w64(info + 0x70, low_copy(&c.spki_params));
        w32(info + 0x78, c.pubkey.len() as u32);
        w64(info + 0x80, low_copy(&c.pubkey));
        w32(info + 0x88, 0);
        let ctx = heap_alloc(0x28, true) as u64;
        w32(ctx, 0x10001); // X509_ASN_ENCODING | PKCS_7_ASN_ENCODING
        w64(ctx + 0x08, low_copy(&c.der));
        w32(ctx + 0x10, c.der.len() as u32);
        w64(ctx + 0x18, info);
        w64(ctx + 0x20, store);
        ctx
    }
}

enum Obj {
    Store(Vec<u64>),
    Prov,
    Key { n: Vec<u8>, e: Vec<u8> },
    Hash { alg: u32, data: Vec<u8> },
}
static OBJS: Mutex<Option<(HashMap<u64, Obj>, u64)>> = Mutex::new(None);
fn new_obj(o: Obj) -> u64 {
    let mut g = OBJS.lock().unwrap();
    let (m, next) = g.get_or_insert_with(|| (HashMap::new(), 0x5000));
    *next += 0x10;
    m.insert(*next, o);
    *next
}

// ------------------------------------------------------------------ CRYPT32

unsafe extern "win64" fn CryptQueryObject(typ: A, obj: A, _ec: A, _ef: A, _fl: A, enc: A, ctype: A, fmt: A, store: A, msg: A, _ctx: A) -> A {
    let path = if typ == 1 { unsafe { crate::strings::wstr(obj) } } else { String::new() };
    let host = crate::files::read_path(&path);
    let Ok(d) = std::fs::read(&host) else {
        hlog!("CryptQueryObject({path}): cannot read {}", host.display());
        return 0;
    };
    let pe = crate::pe::Pe::parse(&d).ok();
    // security directory (data directory 4): file offset + size, WIN_CERTIFICATE header 8 bytes
    let nt = u32::from_le_bytes(d[0x3c..0x40].try_into().unwrap()) as usize;
    let dd = nt + 24 + 112 + 4 * 8;
    let off = u32::from_le_bytes(d[dd..dd + 4].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(d[dd + 4..dd + 8].try_into().unwrap()) as usize;
    let _ = pe;
    if off == 0 || len < 8 {
        hlog!("CryptQueryObject({path}): no Authenticode signature");
        return 0;
    }
    let certs = parse_pkcs7_certs(&d[off + 8..off + len]);
    let st = new_obj(Obj::Store(Vec::new()));
    let ctxs: Vec<u64> = certs.iter().map(|c| make_cert_context(c, st)).collect();
    hlog!("CryptQueryObject({path}): PKCS#7 with {} certificates (serials LE: {})", certs.len(),
        certs.iter().map(|c| c.serial_le.iter().map(|b| format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>().join(", "));
    if let Some((m, _)) = OBJS.lock().unwrap().as_mut() {
        m.insert(st, Obj::Store(ctxs));
    }
    unsafe {
        if enc != 0 {
            (enc as *mut u32).write_unaligned(0x10001);
        }
        if ctype != 0 {
            (ctype as *mut u32).write_unaligned(10); // CERT_QUERY_CONTENT_PKCS7_SIGNED_EMBED
        }
        if fmt != 0 {
            (fmt as *mut u32).write_unaligned(1);
        }
        if store != 0 {
            (store as *mut u64).write_unaligned(st);
        }
        if msg != 0 {
            (msg as *mut u64).write_unaligned(new_obj(Obj::Prov));
        }
    }
    1
}

unsafe extern "win64" fn CertFindCertificateInStore(store: A, _enc: A, _fl: A, typ: A, _para: A, prev: A) -> A {
    if typ & 0xFFFF_FFFF != 0 {
        hlog!("CertFindCertificateInStore: find type {typ:#x} not supported");
        return 0;
    }
    let g = OBJS.lock().unwrap();
    let Some(Obj::Store(v)) = g.as_ref().and_then(|(m, _)| m.get(&store)) else { return 0 };
    let i = if prev == 0 { 0 } else { v.iter().position(|&c| c == prev).map(|p| p + 1).unwrap_or(v.len()) };
    v.get(i).copied().unwrap_or(0)
}

unsafe extern "win64" fn CertGetCertificateChain(_eng: A, cert: A, _time: A, _store: A, _para: A, _fl: A, _res: A, out: A) -> A {
    unsafe {
        let info = (cert as *const u64).add(3).read_unaligned();
        let not_after = ((info + 0x48) as *const u64).read_unaligned();
        let not_before = ((info + 0x40) as *const u64).read_unaligned();
        let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        let now = crate::files::unix_to_filetime(d.as_secs() as i64, 0);
        // only the time check is evaluated (the client accepts NOT_TIME_VALID); 1 = CERT_TRUST_IS_NOT_TIME_VALID
        let err: u32 = if now > not_after || now < not_before { 1 } else { 0 };
        let elem = heap_alloc(0x40, true) as u64;
        (elem as *mut u32).write_unaligned(0x40);
        ((elem + 8) as *mut u64).write_unaligned(cert);
        ((elem + 0x10) as *mut u32).write_unaligned(err);
        let rg_e = heap_alloc(8, true) as u64;
        (rg_e as *mut u64).write_unaligned(elem);
        let simple = heap_alloc(0x40, true) as u64;
        (simple as *mut u32).write_unaligned(0x40);
        ((simple + 4) as *mut u32).write_unaligned(err);
        ((simple + 0xC) as *mut u32).write_unaligned(1);
        ((simple + 0x10) as *mut u64).write_unaligned(rg_e);
        let rg_s = heap_alloc(8, true) as u64;
        (rg_s as *mut u64).write_unaligned(simple);
        let chain = heap_alloc(0x40, true) as u64;
        (chain as *mut u32).write_unaligned(0x40);
        ((chain + 4) as *mut u32).write_unaligned(err);
        ((chain + 0xC) as *mut u32).write_unaligned(1);
        ((chain + 0x10) as *mut u64).write_unaligned(rg_s);
        (out as *mut u64).write_unaligned(chain);
        hlog!("CertGetCertificateChain: certificate valid until FILETIME {not_after:#x}; error status {err:#x} (1 = NOT_TIME_VALID)");
    }
    1
}
unsafe extern "win64" fn CertFreeCertificateChain(_c: A) -> A {
    0
}
unsafe extern "win64" fn CertVerifyCertificateChainPolicy(_p: A, _c: A, _para: A, status: A) -> A {
    unsafe { std::ptr::write_bytes((status + 4) as *mut u8, 0, 8) };
    1
}
unsafe extern "win64" fn CertFreeCertificateContext(_c: A) -> A {
    1
}
unsafe extern "win64" fn CertCloseStore(_s: A, _f: A) -> A {
    1
}

unsafe extern "win64" fn CryptImportPublicKeyInfo(_prov: A, _enc: A, info: A, out: A) -> A {
    unsafe {
        let cb = ((info + 0x18) as *const u32).read_unaligned() as usize;
        let pb = ((info + 0x20) as *const u64).read_unaligned();
        let der = std::slice::from_raw_parts(pb as *const u8, cb);
        // RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }
        let Some((_, c, l)) = tlv(der, 0) else { return 0 };
        let f = children(der, c, l);
        let strip = |b: &[u8]| -> Vec<u8> {
            let mut v = b.to_vec();
            while v.len() > 1 && v[0] == 0 {
                v.remove(0);
            }
            v
        };
        let n = strip(&der[f[0].2..f[0].2 + f[0].3]);
        let e = strip(&der[f[1].2..f[1].2 + f[1].3]);
        hlog!("CryptImportPublicKeyInfo: RSA-{} key, e = {}", n.len() * 8, e.iter().fold(0u64, |a, &b| (a << 8) | b as u64));
        (out as *mut u64).write_unaligned(new_obj(Obj::Key { n, e }));
    }
    1
}

unsafe extern "win64" fn CryptBinaryToStringA(pb: A, cb: A, flags: A, out: A, pcch: A) -> A {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if flags & 0xFFFF != 1 {
        hlog!("CryptBinaryToStringA: flags {flags:#x} not supported");
        return 0;
    }
    let data = unsafe { std::slice::from_raw_parts(pb as *const u8, cb as u32 as usize) };
    let mut s = Vec::new();
    let mut line = 0;
    for ch in data.chunks(3) {
        let v = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= ch.len() {
                s.push(T[((v >> (18 - 6 * k)) & 63) as usize]);
            } else {
                s.push(b'=');
            }
        }
        line += 4;
        if line == 64 {
            s.extend_from_slice(b"\r\n");
            line = 0;
        }
    }
    if line != 0 {
        s.extend_from_slice(b"\r\n");
    }
    unsafe {
        if out == 0 {
            (pcch as *mut u32).write_unaligned(s.len() as u32 + 1);
            return 1;
        }
        let cap = (pcch as *const u32).read_unaligned() as usize;
        if cap < s.len() + 1 {
            return 0;
        }
        std::ptr::copy_nonoverlapping(s.as_ptr(), out as *mut u8, s.len());
        *((out as *mut u8).add(s.len())) = 0;
        (pcch as *mut u32).write_unaligned(s.len() as u32);
    }
    1
}

// ------------------------------------------------------------------ ADVAPI32

unsafe extern "win64" fn CryptAcquireContextA(out: A, _c: A, _p: A, _t: A, _f: A) -> A {
    unsafe { (out as *mut u64).write_unaligned(new_obj(Obj::Prov)) };
    1
}
unsafe extern "win64" fn CryptReleaseContext(_p: A, _f: A) -> A {
    1
}
unsafe extern "win64" fn CryptCreateHash(_p: A, alg: A, _k: A, _f: A, out: A) -> A {
    let alg = alg as u32;
    if alg != 0x8004 {
        hlog!("CryptCreateHash: algorithm {alg:#x} not supported (only CALG_SHA1)");
        return 0;
    }
    unsafe { (out as *mut u64).write_unaligned(new_obj(Obj::Hash { alg, data: Vec::new() })) };
    1
}
unsafe extern "win64" fn CryptHashData(h: A, pb: A, len: A, _f: A) -> A {
    let bytes = unsafe { std::slice::from_raw_parts(pb as *const u8, len as u32 as usize) };
    let mut g = OBJS.lock().unwrap();
    if let Some(Obj::Hash { data, .. }) = g.as_mut().and_then(|(m, _)| m.get_mut(&h)) {
        data.extend_from_slice(bytes);
        return 1;
    }
    0
}
unsafe extern "win64" fn CryptDestroyHash(h: A) -> A {
    if let Some((m, _)) = OBJS.lock().unwrap().as_mut() {
        m.remove(&h);
    }
    1
}
unsafe extern "win64" fn CryptDestroyKey(_h: A) -> A {
    1
}

static VERIFY_COUNT: Mutex<(u32, u32)> = Mutex::new((0, 0));

unsafe extern "win64" fn CryptVerifySignatureA(h: A, sig: A, siglen: A, key: A, _desc: A, _fl: A) -> A {
    let sig_le = unsafe { std::slice::from_raw_parts(sig as *const u8, siglen as u32 as usize) }.to_vec();
    let g = OBJS.lock().unwrap();
    let m = &g.as_ref().unwrap().0;
    let (Some(Obj::Hash { alg: _, data }), Some(Obj::Key { n, e })) = (m.get(&h), m.get(&key)) else {
        hlog!("CryptVerifySignatureA: bad handles");
        return 0;
    };
    let digest = sha1(data);
    let n_big = rsa::BigUint::from_bytes_be(n);
    let s_be: Vec<u8> = sig_le.iter().rev().copied().collect();
    let s = rsa::BigUint::from_bytes_be(&s_be);
    let em = s.modpow(&rsa::BigUint::from_bytes_be(e), &n_big).to_bytes_be();
    // EMSA-PKCS1-v1_5: 00 01 FF..FF 00 DigestInfo(SHA-1) H  (to_bytes_be drops the leading 00)
    const DI: [u8; 15] = [0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04, 0x14];
    let k = n.len();
    let mut want = vec![0x01u8];
    want.resize(k - 1 - 1 - DI.len() - 20, 0xFF);
    want.push(0);
    want.extend_from_slice(&DI);
    want.extend_from_slice(&digest);
    let ok = em == want;
    let mut c = VERIFY_COUNT.lock().unwrap();
    if ok {
        c.0 += 1;
    } else {
        c.1 += 1;
    }
    if c.0 + c.1 <= 3 || !ok {
        hlog!("CryptVerifySignatureA: SHA-1 of {} bytes, RSA-{} signature -> {}", data.len(), k * 8, if ok { "VALID" } else { "INVALID" });
    }
    if !ok {
        crate::teb::set_last_error(0x8009_0006); // NTE_BAD_SIGNATURE
    }
    ok as A
}

pub fn verify_counts() -> (u32, u32) {
    *VERIFY_COUNT.lock().unwrap()
}

// ------------------------------------------------------------------ SHA-1

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

macro_rules! t {
    ($dll:expr; $($name:ident),* $(,)?) => { vec![$(($dll, stringify!($name), $name as *const () as usize)),*] };
}

pub fn table() -> Vec<(&'static str, &'static str, usize)> {
    let mut v = t!["crypt32.dll"; CryptQueryObject, CertFindCertificateInStore, CertGetCertificateChain, CertFreeCertificateChain,
        CertVerifyCertificateChainPolicy, CertFreeCertificateContext, CertCloseStore, CryptImportPublicKeyInfo, CryptBinaryToStringA];
    v.extend(t!["advapi32.dll"; CryptAcquireContextA, CryptReleaseContext, CryptCreateHash, CryptHashData, CryptDestroyHash,
        CryptDestroyKey, CryptVerifySignatureA]);
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha1_known_answers() {
        let hex = |b: [u8; 20]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(super::sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(super::sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }
}
