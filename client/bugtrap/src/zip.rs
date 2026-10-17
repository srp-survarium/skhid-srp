// The ZIP of a crash dump: CRC-32, a DEFLATE writer (RFC 1951), a DEFLATE
// reader that only checks a stream, and the ZIP records (APPNOTE 6.3) of an
// archive of one file. No Windows call and no allocation: the caller passes
// the memory and reads and writes the files. Tested on any host:
//   rustc --edition 2024 --test src/zip.rs -o target/zip-test
#![allow(dead_code)]

/// History a match may reach back into, and the size of the reader's window.
pub const WINDOW: usize = 32 * 1024;
/// New input per block. A block that would not be smaller than its bytes is
/// stored instead, so the output of one block is at most CHUNK + OUT_SLACK.
pub const CHUNK: usize = 256 * 1024;
const OUT_SLACK: usize = 1024;
const HASH_BITS: u32 = 15;
const HASH: usize = 1 << HASH_BITS;
/// Candidates tried per position (greedy matching, zlib's levels 1 to 2 or so):
/// a 190 MB dump takes a few seconds on one core.
const MAX_CHAIN: u32 = 8;
const NICE: usize = 128;
const MAX_MATCH: usize = 258;

/// Bytes of memory `Memory::new` carves up (in 4-byte words).
pub const MEMORY_WORDS: usize =
    (WINDOW + CHUNK) / 4 + HASH + WINDOW + CHUNK + (CHUNK + OUT_SLACK) / 4;

/// Errors besides the caller's own Windows codes, which pass through.
pub const ERROR_INVALID_DATA: u32 = 13;
pub const ERROR_FILE_TOO_LARGE: u32 = 223;
/// The input may not reach 4 GiB: positions and ZIP sizes are 32-bit.
pub const MAX_INPUT: u64 = 0xF000_0000;

pub trait Source {
    /// Fills part of `buf`; 0 at the end.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, u32>;
}
pub trait Sink {
    fn write(&mut self, data: &[u8]) -> Result<(), u32>;
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}
static CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 of ZIP, continued from `crc` (0 to start).
pub fn crc32(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

const fn length_codes() -> [u8; 259] {
    let mut table = [0u8; 259];
    let mut c = 0;
    while c < 29 {
        let mut l = LENGTH_BASE[c] as usize;
        while l < LENGTH_BASE[c] as usize + (1 << LENGTH_EXTRA[c]) && l <= 258 {
            table[l] = c as u8;
            l += 1;
        }
        c += 1;
    }
    table
}
static LENGTH_CODE: [u8; 259] = length_codes();
// Codes of distances 1..=256 by distance - 1, then of the others by (distance - 1) >> 7.
const fn dist_codes() -> [u8; 512] {
    let mut table = [0u8; 512];
    let mut c = 0;
    while c < 30 {
        let mut d = DIST_BASE[c] as usize;
        while d < DIST_BASE[c] as usize + (1 << DIST_EXTRA[c]) {
            if d <= 256 {
                table[d - 1] = c as u8;
            } else {
                table[256 + ((d - 1) >> 7)] = c as u8;
            }
            d += 1;
        }
        c += 1;
    }
    table
}
static DIST_CODE: [u8; 512] = dist_codes();
fn dist_code(d: usize) -> usize {
    if d <= 256 {
        DIST_CODE[d - 1] as usize
    } else {
        DIST_CODE[256 + ((d - 1) >> 7)] as usize
    }
}

/// The writer's memory: history and new input, the match finder's tables, one
/// block's symbols and one block's output.
pub struct Memory<'a> {
    data: &'a mut [u8],
    head: &'a mut [u32],
    prev: &'a mut [u32],
    tokens: &'a mut [u32],
    out: &'a mut [u8],
}
impl<'a> Memory<'a> {
    /// Carves MEMORY_WORDS words (zeroed or not).
    pub fn new(words: &'a mut [u32]) -> Self {
        let (data, rest) = words.split_at_mut((WINDOW + CHUNK) / 4);
        let (head, rest) = rest.split_at_mut(HASH);
        let (prev, rest) = rest.split_at_mut(WINDOW);
        let (tokens, out) = rest.split_at_mut(CHUNK);
        let bytes = |w: &'a mut [u32]| {
            // u32 has no padding and any byte pattern is a valid u8.
            unsafe { core::slice::from_raw_parts_mut(w.as_mut_ptr().cast::<u8>(), w.len() * 4) }
        };
        head.fill(0);
        Self {
            data: bytes(data),
            head,
            prev,
            tokens,
            out: bytes(out),
        }
    }
    /// The reader needs only a window and an input buffer.
    fn bytes(&mut self) -> &mut [u8] {
        self.data
    }
}

struct Bits<'a> {
    out: &'a mut [u8],
    n: usize,
    acc: u64,
    count: u32,
}
impl Bits<'_> {
    #[inline]
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= (value as u64) << self.count;
        self.count += bits;
        while self.count >= 8 {
            self.out[self.n] = self.acc as u8;
            self.n += 1;
            self.acc >>= 8;
            self.count -= 8;
        }
    }
    fn align(&mut self) {
        if self.count > 0 {
            self.put(0, 8 - self.count);
        }
    }
}

/// Huffman code lengths of at most `limit` bits for `freq`; unused symbols get
/// 0. At least two symbols must be used. Frequencies are halved until the
/// longest code fits (all equal frequencies give a balanced tree).
fn code_lengths(freq: &[u32], limit: u32, lengths: &mut [u8]) {
    let n = freq.len();
    let mut f = [0u32; 288];
    f[..n].copy_from_slice(freq);
    loop {
        let mut leaves = [0u16; 288];
        let mut m = 0;
        for s in 0..n {
            if f[s] != 0 {
                // Insertion sort by frequency, then by symbol.
                let mut k = m;
                while k > 0 && f[leaves[k - 1] as usize] > f[s] {
                    leaves[k] = leaves[k - 1];
                    k -= 1;
                }
                leaves[k] = s as u16;
                m += 1;
            }
        }
        lengths[..n].fill(0);
        if m < 2 {
            if m == 1 {
                lengths[leaves[0] as usize] = 1;
            }
            return;
        }
        // Two queues: sorted leaves, and inner nodes in the order they are made
        // (their weights only grow). Node i < m is leaf i, inner nodes follow.
        let mut weight = [0u64; 576];
        let mut parent = [0u16; 576];
        for k in 0..m {
            weight[k] = f[leaves[k] as usize] as u64;
        }
        let (mut leaf, mut inner, mut next) = (0, m, m);
        while next < 2 * m - 1 {
            let mut pick = || {
                if leaf < m && (inner >= next || weight[leaf] <= weight[inner]) {
                    leaf += 1;
                    leaf - 1
                } else {
                    inner += 1;
                    inner - 1
                }
            };
            let (a, b) = (pick(), pick());
            weight[next] = weight[a] + weight[b];
            parent[a] = next as u16;
            parent[b] = next as u16;
            next += 1;
        }
        // Parents come after their children: depths from the root down.
        let mut depth = [0u16; 576];
        let mut longest = 0;
        for i in (0..2 * m - 2).rev() {
            depth[i] = depth[parent[i] as usize] + 1;
            if i < m && depth[i] as u32 > longest {
                longest = depth[i] as u32;
            }
        }
        if longest <= limit {
            for k in 0..m {
                lengths[leaves[k] as usize] = depth[k] as u8;
            }
            return;
        }
        for v in f[..n].iter_mut() {
            if *v != 0 {
                *v = (*v + 1) >> 1;
            }
        }
    }
}

/// Canonical codes (RFC 1951 3.2.2), bit-reversed for an LSB-first writer.
fn canonical(lengths: &[u8], codes: &mut [u16]) {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut next = [0u16; 16];
    let mut code = 0u16;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    for (s, &l) in lengths.iter().enumerate() {
        if l != 0 {
            let c = next[l as usize];
            next[l as usize] += 1;
            codes[s] = c.reverse_bits() >> (16 - l as u32);
        }
    }
}

fn at_least_two(freq: &mut [u32]) {
    let used = freq.iter().filter(|&&f| f != 0).count();
    if used < 2 {
        for s in 0..2 {
            if freq[s] == 0 && freq.iter().filter(|&&f| f != 0).count() < 2 {
                freq[s] = 1;
            }
        }
    }
}

const MATCH: u32 = 1 << 31;

pub struct Summary {
    pub crc: u32,
    pub size: u64,
    pub packed: u64,
}

/// Compresses all of `src` into `dst` as one raw DEFLATE stream.
pub fn deflate(src: &mut dyn Source, dst: &mut dyn Sink, mem: &mut Memory) -> Result<Summary, u32> {
    let mut sum = Summary {
        crc: 0,
        size: 0,
        packed: 0,
    };
    // data[0..start] is the history, data[start..end] the new input. Position p
    // of the stream is data[p - base]; the hash tables hold positions + 1.
    let mut base: u32 = 0;
    let mut start = 0usize;
    let mut acc = 0u64;
    let mut count = 0u32;
    loop {
        let mut end = start;
        while end < start + CHUNK {
            let n = src.read(&mut mem.data[end..start + CHUNK])?;
            if n == 0 {
                break;
            }
            sum.crc = crc32(sum.crc, &mem.data[end..end + n]);
            end += n;
        }
        sum.size += (end - start) as u64;
        if sum.size > MAX_INPUT {
            return Err(ERROR_FILE_TOO_LARGE);
        }
        let mut bits = Bits {
            out: &mut *mem.out,
            n: 0,
            acc,
            count,
        };
        if end > start {
            block(
                mem.data, mem.head, mem.prev, mem.tokens, &mut bits, base, start, end,
            );
        } else {
            // The last block: empty and stored.
            bits.put(1, 3);
            bits.align();
            bits.put(0, 16);
            bits.put(0xffff, 16);
            bits.align();
        }
        let (n, last) = (bits.n, end == start);
        acc = bits.acc;
        count = bits.count;
        dst.write(&mem.out[..n])?;
        sum.packed += n as u64;
        if last {
            return Ok(sum);
        }
        let keep = end.min(WINDOW);
        mem.data.copy_within(end - keep..end, 0);
        base += (end - keep) as u32;
        start = keep;
    }
}

#[allow(clippy::too_many_arguments)]
fn block(
    data: &[u8],
    head: &mut [u32],
    prev: &mut [u32],
    tokens: &mut [u32],
    bits: &mut Bits,
    base: u32,
    start: usize,
    end: usize,
) {
    let mut lit_freq = [0u32; 286];
    let mut dist_freq = [0u32; 30];
    let mut extra_bits = 0u64;
    let mut ntokens = 0;
    let hash = |i: usize| {
        let v = data[i] as u32 | (data[i + 1] as u32) << 8 | (data[i + 2] as u32) << 16;
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let mask = WINDOW - 1;
    let mut i = start;
    while i < end {
        let (mut best, mut best_dist) = (0usize, 0usize);
        if i + 3 <= end {
            let cur = base + i as u32;
            let h = hash(i);
            let mut cand = head[h];
            prev[cur as usize & mask] = cand;
            head[h] = cur + 1;
            let max = (end - i).min(MAX_MATCH);
            let mut chain = MAX_CHAIN;
            while cand != 0 && chain > 0 {
                let c = cand - 1;
                let dist = (cur - c) as usize;
                if dist > WINDOW {
                    break;
                }
                let j = (c - base) as usize;
                if data[j + best] == data[i + best] {
                    let mut l = 0;
                    while l < max && data[j + l] == data[i + l] {
                        l += 1;
                    }
                    if l > best {
                        best = l;
                        best_dist = dist;
                        if l >= NICE || l >= max {
                            break;
                        }
                    }
                }
                let older = prev[c as usize & mask];
                // An entry overwritten by a newer position ends the chain.
                if older >= cand {
                    break;
                }
                cand = older;
                chain -= 1;
            }
        }
        if best >= 3 {
            let lc = LENGTH_CODE[best] as usize;
            let dc = dist_code(best_dist);
            lit_freq[257 + lc] += 1;
            dist_freq[dc] += 1;
            extra_bits += (LENGTH_EXTRA[lc] + DIST_EXTRA[dc]) as u64;
            tokens[ntokens] = MATCH | (best as u32) << 16 | (best_dist - 1) as u32;
            ntokens += 1;
            for k in i + 1..i + best {
                if k + 3 > end {
                    break;
                }
                let p = base + k as u32;
                let h = hash(k);
                prev[p as usize & mask] = head[h];
                head[h] = p + 1;
            }
            i += best;
        } else {
            lit_freq[data[i] as usize] += 1;
            tokens[ntokens] = data[i] as u32;
            ntokens += 1;
            i += 1;
        }
    }
    lit_freq[256] = 1;
    at_least_two(&mut dist_freq);
    let mut lit_len = [0u8; 286];
    let mut dist_len = [0u8; 30];
    code_lengths(&lit_freq, 15, &mut lit_len);
    code_lengths(&dist_freq, 15, &mut dist_len);
    let hlit = 257
        + lit_len[257..]
            .iter()
            .rposition(|&l| l != 0)
            .map_or(0, |p| p + 1);
    let hdist = 1 + dist_len[1..]
        .iter()
        .rposition(|&l| l != 0)
        .map_or(0, |p| p + 1);
    // The code lengths, run-length coded (RFC 1951 3.2.7).
    let mut seq = [0u8; 316];
    seq[..hlit].copy_from_slice(&lit_len[..hlit]);
    seq[hlit..hlit + hdist].copy_from_slice(&dist_len[..hdist]);
    let total = hlit + hdist;
    let mut rle = [(0u8, 0u8); 316];
    let mut nrle = 0;
    let mut cl_freq = [0u32; 19];
    let mut emit = |sym: u8, extra: u8| {
        rle[nrle] = (sym, extra);
        nrle += 1;
        cl_freq[sym as usize] += 1;
    };
    let mut k = 0;
    while k < total {
        let l = seq[k];
        let mut run = 1;
        while k + run < total && seq[k + run] == l {
            run += 1;
        }
        k += run;
        if l == 0 {
            while run >= 3 {
                let r = if run >= 11 { run.min(138) } else { run };
                if r >= 11 {
                    emit(18, (r - 11) as u8)
                } else {
                    emit(17, (r - 3) as u8)
                }
                run -= r;
            }
        } else {
            emit(l, 0);
            run -= 1;
            while run >= 3 {
                let r = run.min(6);
                emit(16, (r - 3) as u8);
                run -= r;
            }
        }
        for _ in 0..run {
            emit(l, 0);
        }
    }
    at_least_two(&mut cl_freq);
    let mut cl_len = [0u8; 19];
    code_lengths(&cl_freq, 7, &mut cl_len);
    let hclen = 4.max(
        19 - CODE_ORDER
            .iter()
            .rev()
            .take_while(|&&s| cl_len[s] == 0)
            .count(),
    );

    let cl_extra = |s: u8| match s {
        16 => 2,
        17 => 3,
        18 => 7,
        _ => 0,
    };
    let mut cost: u64 = 3 + 14 + 3 * hclen as u64 + extra_bits;
    for &(s, _) in &rle[..nrle] {
        cost += (cl_len[s as usize] + cl_extra(s)) as u64;
    }
    for s in 0..286 {
        cost += lit_freq[s] as u64 * lit_len[s] as u64;
    }
    for s in 0..30 {
        cost += dist_freq[s] as u64 * dist_len[s] as u64;
    }
    let size = end - start;
    let pieces = size.div_ceil(65535) as u64;
    if (size as u64) * 8 + pieces * 42 <= cost {
        for piece in data[start..end].chunks(65535) {
            bits.put(0, 3);
            bits.align();
            bits.put(piece.len() as u32, 16);
            bits.put(!(piece.len() as u32) & 0xffff, 16);
            bits.out[bits.n..bits.n + piece.len()].copy_from_slice(piece);
            bits.n += piece.len();
        }
        return;
    }
    let mut lit_code = [0u16; 286];
    let mut dist_code_ = [0u16; 30];
    let mut cl_code = [0u16; 19];
    canonical(&lit_len, &mut lit_code);
    canonical(&dist_len, &mut dist_code_);
    canonical(&cl_len, &mut cl_code);
    bits.put(2 << 1, 3);
    bits.put((hlit - 257) as u32, 5);
    bits.put((hdist - 1) as u32, 5);
    bits.put((hclen - 4) as u32, 4);
    for &s in &CODE_ORDER[..hclen] {
        bits.put(cl_len[s] as u32, 3);
    }
    for &(s, e) in &rle[..nrle] {
        bits.put(cl_code[s as usize] as u32, cl_len[s as usize] as u32);
        bits.put(e as u32, cl_extra(s) as u32);
    }
    for &t in &tokens[..ntokens] {
        if t & MATCH == 0 {
            bits.put(lit_code[t as usize] as u32, lit_len[t as usize] as u32);
        } else {
            let len = ((t >> 16) & 0x1ff) as usize;
            let dist = (t & 0xffff) as usize + 1;
            let lc = LENGTH_CODE[len] as usize;
            bits.put(lit_code[257 + lc] as u32, lit_len[257 + lc] as u32);
            bits.put(
                (len - LENGTH_BASE[lc] as usize) as u32,
                LENGTH_EXTRA[lc] as u32,
            );
            let dc = dist_code(dist);
            bits.put(dist_code_[dc] as u32, dist_len[dc] as u32);
            bits.put(
                (dist - DIST_BASE[dc] as usize) as u32,
                DIST_EXTRA[dc] as u32,
            );
        }
    }
    bits.put(lit_code[256] as u32, lit_len[256] as u32);
}

/// Decoding table of one canonical code (as in zlib's puff.c).
struct Huffman {
    count: [u16; 16],
    symbol: [u16; 288],
}
/// The longest code that may leave bit patterns unused. The check must refuse
/// what zlib (and so the players' unzip tools) refuses: a code-length code
/// must be complete; a dynamic block's literal/length or distance code too,
/// unless it is a single code of one bit. Only the fixed codes have unused
/// patterns (literal/lengths 286, 287 and distances 30, 31), which are refused
/// when they are decoded.
const COMPLETE: usize = 0;
const ONE_BIT: usize = 1;
const FIXED: usize = 15;
impl Huffman {
    fn new(lengths: &[u8], unused_up_to: usize) -> Result<Self, u32> {
        let mut h = Huffman {
            count: [0; 16],
            symbol: [0; 288],
        };
        for &l in lengths {
            h.count[l as usize] += 1;
        }
        let mut left: i32 = 1;
        let mut longest = 0;
        for len in 1..16 {
            left = (left << 1) - h.count[len] as i32;
            if left < 0 {
                return Err(ERROR_INVALID_DATA);
            }
            if h.count[len] != 0 {
                longest = len;
            }
        }
        // No code at all is left to the decoding (zlib does the same): a
        // stream that needs one is refused there.
        if left > 0 && longest > unused_up_to {
            return Err(ERROR_INVALID_DATA);
        }
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + h.count[len];
        }
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                h.symbol[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(h)
    }
}

struct Reader<'a> {
    src: &'a mut dyn Source,
    buf: &'a mut [u8],
    pos: usize,
    len: usize,
    acc: u32,
    count: u32,
    ended: bool,
}
impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, u32> {
        if self.pos == self.len {
            self.len = if self.ended {
                0
            } else {
                self.src.read(self.buf)?
            };
            self.pos = 0;
            if self.len == 0 {
                self.ended = true;
                return Err(ERROR_INVALID_DATA);
            }
        }
        self.pos += 1;
        Ok(self.buf[self.pos - 1])
    }
    fn bits(&mut self, n: u32) -> Result<u32, u32> {
        while self.count < n {
            self.acc |= (self.byte()? as u32) << self.count;
            self.count += 8;
        }
        let v = self.acc & ((1u32 << n) - 1);
        self.acc >>= n;
        self.count -= n;
        Ok(v)
    }
    fn decode(&mut self, h: &Huffman) -> Result<usize, u32> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= self.bits(1)? as i32;
            let count = h.count[len] as i32;
            if code - count < first {
                return Ok(h.symbol[(index + code - first) as usize] as usize);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(ERROR_INVALID_DATA)
    }
}

/// Reads a whole raw DEFLATE stream from `src` and gives the CRC-32 and size
/// of what it holds. Bytes after the stream's end are an error.
pub fn check(src: &mut dyn Source, mem: &mut Memory) -> Result<(u32, u64), u32> {
    let (window, buf) = mem.bytes().split_at_mut(WINDOW);
    let mut r = Reader {
        src,
        buf,
        pos: 0,
        len: 0,
        acc: 0,
        count: 0,
        ended: false,
    };
    let mut crc = !0u32;
    let mut size = 0u64;
    let mask = WINDOW - 1;
    let mut at = 0usize;
    #[inline]
    fn put(window: &mut [u8], b: u8, at: &mut usize, crc: &mut u32) {
        window[*at & (WINDOW - 1)] = b;
        *at += 1;
        *crc = CRC_TABLE[((*crc ^ b as u32) & 0xff) as usize] ^ (*crc >> 8);
    }
    loop {
        let last = r.bits(1)?;
        let kind = r.bits(2)?;
        if kind == 0 {
            r.acc = 0;
            r.count = 0;
            let len = r.bits(16)?;
            if r.bits(16)? != !len & 0xffff {
                return Err(ERROR_INVALID_DATA);
            }
            for _ in 0..len {
                let b = r.byte()?;
                put(window, b, &mut at, &mut crc);
            }
            size += len as u64;
        } else if kind == 3 {
            return Err(ERROR_INVALID_DATA);
        } else {
            let mut lengths = [0u8; 320];
            let (hlit, hdist) = if kind == 1 {
                for (s, l) in lengths[..288].iter_mut().enumerate() {
                    *l = match s {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                lengths[288..318].fill(5);
                (288, 30)
            } else {
                let hlit = r.bits(5)? as usize + 257;
                let hdist = r.bits(5)? as usize + 1;
                let hclen = r.bits(4)? as usize + 4;
                if hlit > 286 || hdist > 30 {
                    return Err(ERROR_INVALID_DATA);
                }
                let mut cl = [0u8; 19];
                for &s in &CODE_ORDER[..hclen] {
                    cl[s] = r.bits(3)? as u8;
                }
                let h = Huffman::new(&cl, COMPLETE)?;
                let mut k = 0;
                while k < hlit + hdist {
                    let s = r.decode(&h)?;
                    let (value, times) = match s {
                        0..=15 => (s as u8, 1),
                        16 if k > 0 => (lengths[k - 1], 3 + r.bits(2)? as usize),
                        17 => (0, 3 + r.bits(3)? as usize),
                        18 => (0, 11 + r.bits(7)? as usize),
                        _ => return Err(ERROR_INVALID_DATA),
                    };
                    if k + times > hlit + hdist {
                        return Err(ERROR_INVALID_DATA);
                    }
                    lengths[k..k + times].fill(value);
                    k += times;
                }
                if lengths[256] == 0 {
                    return Err(ERROR_INVALID_DATA);
                }
                (hlit, hdist)
            };
            let unused = if kind == 1 { FIXED } else { ONE_BIT };
            let lit = Huffman::new(&lengths[..hlit], unused)?;
            let dist = Huffman::new(&lengths[hlit..hlit + hdist], unused)?;
            loop {
                let s = r.decode(&lit)?;
                if s < 256 {
                    put(window, s as u8, &mut at, &mut crc);
                    size += 1;
                } else if s == 256 {
                    break;
                } else {
                    let s = s - 257;
                    if s >= 29 {
                        return Err(ERROR_INVALID_DATA);
                    }
                    let len = LENGTH_BASE[s] as usize + r.bits(LENGTH_EXTRA[s] as u32)? as usize;
                    let d = r.decode(&dist)?;
                    if d >= 30 {
                        return Err(ERROR_INVALID_DATA);
                    }
                    let d = DIST_BASE[d] as usize + r.bits(DIST_EXTRA[d] as u32)? as usize;
                    if d as u64 > size || d > WINDOW {
                        return Err(ERROR_INVALID_DATA);
                    }
                    for _ in 0..len {
                        let b = window[at.wrapping_sub(d) & mask];
                        put(window, b, &mut at, &mut crc);
                    }
                    size += len as u64;
                }
            }
        }
        if last == 1 {
            break;
        }
    }
    // Nothing may follow the last block but the rest of its last byte.
    if r.pos != r.len || (!r.ended && r.src.read(r.buf)? != 0) {
        return Err(ERROR_INVALID_DATA);
    }
    Ok((!crc, size))
}

/// MS-DOS time and date of the ZIP records.
pub fn dos_time(
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
) -> (u16, u16) {
    (
        hour << 11 | minute << 5 | second / 2,
        (year.max(1980) - 1980) << 9 | month << 5 | day,
    )
}

/// The fields every record repeats. `name` is plain ASCII (flag bit 11 clear).
pub struct Entry<'a> {
    pub name: &'a [u8],
    pub time: u16,
    pub date: u16,
    pub crc: u32,
    pub packed: u32,
    pub size: u32,
}
const HEADER: usize = 30;
const CENTRAL: usize = 46;
const END: usize = 22;
pub const MAX_NAME: usize = 128;
pub const LOCAL_MAX: usize = HEADER + MAX_NAME;
/// The central directory and the end record.
pub const TAIL_MAX: usize = CENTRAL + MAX_NAME + END;

fn le(out: &mut [u8], at: &mut usize, v: u32, n: usize) {
    out[*at..*at + n].copy_from_slice(&v.to_le_bytes()[..n]);
    *at += n;
}
fn common(e: &Entry, out: &mut [u8], at: &mut usize) {
    le(out, at, 20, 2); // version needed: 2.0, deflate
    le(out, at, 0, 2); // flags: no data descriptor, name not UTF-8
    le(out, at, 8, 2); // deflate
    le(out, at, e.time as u32, 2);
    le(out, at, e.date as u32, 2);
    le(out, at, e.crc, 4);
    le(out, at, e.packed, 4);
    le(out, at, e.size, 4);
    le(out, at, e.name.len() as u32, 2);
    le(out, at, 0, 2); // no extra field
}
/// The local header, at offset 0. Returns its length.
pub fn local_header(e: &Entry, out: &mut [u8; LOCAL_MAX]) -> usize {
    let mut at = 0;
    le(out, &mut at, 0x0403_4b50, 4);
    common(e, out, &mut at);
    out[at..at + e.name.len()].copy_from_slice(e.name);
    at + e.name.len()
}
/// The central directory entry and the end record, at offset `offset`.
pub fn tail(e: &Entry, offset: u32, out: &mut [u8; TAIL_MAX]) -> usize {
    let mut at = 0;
    le(out, &mut at, 0x0201_4b50, 4);
    le(out, &mut at, 20, 2); // made by: MS-DOS, 2.0
    common(e, out, &mut at);
    le(out, &mut at, 0, 2); // comment
    le(out, &mut at, 0, 2); // disk
    le(out, &mut at, 0, 2); // internal attributes
    le(out, &mut at, 0x20, 4); // FILE_ATTRIBUTE_ARCHIVE
    le(out, &mut at, 0, 4); // local header offset
    out[at..at + e.name.len()].copy_from_slice(e.name);
    at += e.name.len();
    let central = at as u32;
    le(out, &mut at, 0x0605_4b50, 4);
    le(out, &mut at, 0, 4); // disks
    le(out, &mut at, 1, 2);
    le(out, &mut at, 1, 2);
    le(out, &mut at, central, 4);
    le(out, &mut at, offset, 4);
    le(out, &mut at, 0, 2); // comment
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, process::Command, vec, vec::Vec};

    struct Slice<'a>(&'a [u8], usize);
    impl Source for Slice<'_> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, u32> {
            // Odd read sizes exercise the block and buffer boundaries.
            let n = buf.len().min(self.0.len()).min(self.1);
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    impl Sink for Vec<u8> {
        fn write(&mut self, data: &[u8]) -> Result<(), u32> {
            self.extend_from_slice(data);
            Ok(())
        }
    }
    fn pack(input: &[u8], step: usize) -> Vec<u8> {
        let mut words = vec![0u32; MEMORY_WORDS];
        let mut out = Vec::new();
        let sum = deflate(
            &mut Slice(input, step),
            &mut out,
            &mut Memory::new(&mut words),
        )
        .unwrap();
        assert_eq!(sum.size, input.len() as u64);
        assert_eq!(sum.crc, crc32(0, input));
        assert_eq!(sum.packed, out.len() as u64);
        let checked = check(&mut Slice(&out, 4093), &mut Memory::new(&mut words)).unwrap();
        assert_eq!(checked, (crc32(0, input), input.len() as u64));
        out
    }
    fn noise(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }
    // Something like a dump: zero pages, repeated structures, text, pointers, noise.
    fn dumpish(n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        let mut k = 0u64;
        while v.len() < n {
            k += 1;
            match k % 5 {
                0 => v.extend(std::iter::repeat_n(0, 4096 * (k as usize % 7))),
                1 => v.extend(noise(3000 + k as usize % 2000, k)),
                2 => (0..700)
                    .for_each(|i| v.extend(&(0x1000_0000u32 + i * 8 + k as u32).to_le_bytes())),
                3 => v.extend(b"xrCore.dll|gamemtl.xr|actor_animation|".repeat(40)),
                _ => v.extend(noise(97, k).repeat(31)),
            }
        }
        v.truncate(n);
        v
    }

    #[test]
    fn crc_of_the_check_string() {
        assert_eq!(crc32(0, b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(crc32(0, b"1234"), b"56789"), 0xCBF4_3926);
        assert_eq!(crc32(0, b""), 0);
    }

    #[test]
    fn round_trips() {
        for input in [
            vec![],
            vec![7],
            vec![0; 1],
            vec![0; 3],
            vec![0; 1 << 20],
            b"abcabcabcabc".repeat(1000),
            noise(CHUNK * 3 + 17, 5),
            dumpish(5 << 20),
            (0..=255u8).cycle().take(CHUNK + 1).collect(),
        ] {
            for step in [1 << 20, 65536 + 7] {
                pack(&input, step);
            }
        }
        let small = pack(&vec![0; 1 << 20], 1 << 20);
        assert!(small.len() < 2000, "{}", small.len());
        let noisy = pack(&noise(1 << 20, 9), 1 << 20);
        assert!(noisy.len() < (1 << 20) + 200, "stored blocks grow little");
    }

    #[test]
    fn check_refuses_damage() {
        let input = dumpish(1 << 20);
        let packed = pack(&input, 1 << 20);
        let mut words = vec![0u32; MEMORY_WORDS];
        for at in [packed.len() / 3, packed.len() / 2, packed.len() - 9] {
            let mut bad = packed.clone();
            bad[at] ^= 0x10;
            let r = check(&mut Slice(&bad, 1 << 20), &mut Memory::new(&mut words));
            assert!(r.map_or(true, |v| v != (crc32(0, &input), input.len() as u64)));
        }
        let mut long = packed.clone();
        long.push(0);
        assert!(check(&mut Slice(&long, 1 << 20), &mut Memory::new(&mut words)).is_err());
        let short = &packed[..packed.len() - 1];
        assert!(check(&mut Slice(short, 1 << 20), &mut Memory::new(&mut words)).is_err());
    }

    /// One last dynamic block of the literals `text`, with the given code
    /// lengths and all 19 code-length code lengths (`cl`, or made to fit).
    fn dynamic(lit: &[u8], dist: &[u8], cl: Option<[u8; 19]>, text: &[u8]) -> Vec<u8> {
        let seq: Vec<u8> = lit.iter().chain(dist).copied().collect();
        let cl_len = cl.unwrap_or_else(|| {
            let mut freq = [0u32; 19];
            seq.iter().for_each(|&l| freq[l as usize] += 1);
            at_least_two(&mut freq);
            let mut cl_len = [0u8; 19];
            code_lengths(&freq, 7, &mut cl_len);
            cl_len
        });
        let (mut cl_code, mut lit_code) = ([0u16; 19], [0u16; 286]);
        canonical(&cl_len, &mut cl_code);
        canonical(lit, &mut lit_code);
        let mut out = vec![0u8; 4096];
        let mut bits = Bits {
            out: &mut out,
            n: 0,
            acc: 0,
            count: 0,
        };
        bits.put(1 | 2 << 1, 3);
        bits.put((lit.len() - 257) as u32, 5);
        bits.put((dist.len() - 1) as u32, 5);
        bits.put(19 - 4, 4);
        for &s in &CODE_ORDER {
            bits.put(cl_len[s] as u32, 3);
        }
        for &l in &seq {
            bits.put(cl_code[l as usize] as u32, cl_len[l as usize] as u32);
        }
        for &b in text.iter().chain(&[0]) {
            let s = if b == 0 { 256 } else { b as usize };
            bits.put(lit_code[s] as u32, lit[s] as u32);
        }
        bits.align();
        let n = bits.n;
        out.truncate(n);
        out
    }

    // The check is the last word before the dump is deleted: it must refuse
    // the codes zlib refuses (an unused bit pattern, but in a single one-bit
    // code), or a damaged ZIP that no unzip tool opens could take the dump's
    // place. And it reads what zlib makes: fixed codes.
    #[test]
    fn check_refuses_what_zlib_refuses() {
        let mut lit = [0u8; 257];
        lit[b'a' as usize] = 1;
        lit[256] = 1;
        let mut lit_short = lit;
        lit_short[256] = 2;
        let text = b"aaa";
        let cases = [
            ("complete codes", dynamic(&lit, &[1, 1], None, text), true),
            (
                "a single one-bit distance code",
                dynamic(&lit, &[1], None, text),
                true,
            ),
            ("no distance code", dynamic(&lit, &[0], None, text), true),
            (
                "an incomplete distance code",
                dynamic(&lit, &[2], None, text),
                false,
            ),
            (
                "an incomplete literal code",
                dynamic(&lit_short, &[1], None, text),
                false,
            ),
            (
                "an incomplete code-length code",
                dynamic(&lit, &[1], Some([5; 19]), text),
                false,
            ),
        ];
        let mut words = vec![0u32; MEMORY_WORDS];
        for (what, stream, valid) in &cases {
            let r = check(&mut Slice(stream, 1 << 20), &mut Memory::new(&mut words));
            let want = Ok((crc32(0, text), text.len() as u64));
            assert_eq!(r == want, *valid, "{what}: {r:?}");
            assert!(*valid || r.is_err(), "{what}: {r:?}");
        }
        // zlib.compressobj(9, DEFLATED, -15, 9, Z_FIXED) of this text.
        let fixed = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0xd7, 0x51, 0xc8, 0x40, 0xa2, 0x14, 0xca, 0xf3, 0x8b,
            0x72, 0x52, 0x00,
        ];
        let text = b"hello, hello, hello world";
        assert_eq!(
            check(&mut Slice(&fixed, 5), &mut Memory::new(&mut words)),
            Ok((crc32(0, text), text.len() as u64))
        );
        // And zlib agrees on each case (when there is a python3 on PATH).
        let dir = std::env::temp_dir().join(format!("srp-inflate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (i, (_, stream, valid)) in cases.iter().enumerate() {
            std::fs::write(dir.join(format!("{i}-{}.deflate", *valid as u8)), stream).unwrap();
        }
        let script = r#"
import pathlib, sys, zlib
for p in sorted(pathlib.Path(sys.argv[1]).glob("*.deflate")):
    try:
        zlib.decompress(p.read_bytes(), -15)
        ok = True
    except zlib.error:
        ok = False
    assert ok == p.stem.endswith("-1"), p.name
print("python zlib: same verdicts")
"#;
        match Command::new("python3")
            .args(["-I", "-c", script, dir.to_str().unwrap()])
            .status()
        {
            Ok(status) => assert!(status.success()),
            Err(_) => eprintln!("no python3: zlib not run"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn code_lengths_fit_their_limit() {
        // Fibonacci frequencies make the deepest possible Huffman tree.
        let mut freq = [0u32; 286];
        let (mut a, mut b) = (1u32, 1u32);
        for f in freq.iter_mut().take(30) {
            *f = a;
            (a, b) = (b, a + b);
        }
        let mut lengths = [0u8; 286];
        code_lengths(&freq, 15, &mut lengths);
        assert!(lengths.iter().all(|&l| l <= 15));
        let kraft: f64 = lengths
            .iter()
            .filter(|&&l| l > 0)
            .map(|&l| 0.5f64.powi(l as i32))
            .sum();
        assert_eq!(kraft, 1.0);
        let mut cl = [0u8; 19];
        code_lengths(&freq[..19], 7, &mut cl);
        assert!(cl.iter().all(|&l| (1..=7).contains(&l)));
    }

    // zlib (Python's) inflates the edge cases too: stored, tiny and empty
    // streams, single-symbol trees, block boundaries.
    #[test]
    fn python_zlib_inflates_them() {
        let dir = std::env::temp_dir().join(format!("srp-deflate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let inputs = [
            vec![],
            vec![7],
            vec![0; 3],
            vec![1; CHUNK + 300],
            b"abcabcabcabc".repeat(1000),
            noise(CHUNK * 2 + 17, 3),
            dumpish(3 << 20),
            (0..=255u8).cycle().take(CHUNK + 1).collect(),
        ];
        for (i, input) in inputs.iter().enumerate() {
            std::fs::write(dir.join(format!("{i}.raw")), input).unwrap();
            std::fs::write(dir.join(format!("{i}.deflate")), pack(input, 65536 + 7)).unwrap();
        }
        let script = r#"
import pathlib, sys, zlib
d = pathlib.Path(sys.argv[1])
for i in range(int(sys.argv[2])):
    assert zlib.decompress((d / f"{i}.deflate").read_bytes(), -15) == (d / f"{i}.raw").read_bytes(), i
print("python zlib: ok")
"#;
        let dir_arg = dir.to_str().unwrap().to_string();
        match Command::new("python3")
            .args(["-I", "-c", script, &dir_arg, &inputs.len().to_string()])
            .status()
        {
            Ok(status) => assert!(status.success()),
            Err(_) => eprintln!("no python3: zlib not run"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // A ZIP made as the DLL makes it, read by Python's zipfile (when there is a
    // python3 on PATH; set SRP_ZIP_SAMPLE to a file, such as a real dump, to use it).
    #[test]
    fn python_reads_the_zip() {
        let input = match std::env::var("SRP_ZIP_SAMPLE") {
            Ok(path) => std::fs::read(path).unwrap(),
            Err(_) => dumpish(40 << 20),
        };
        let started = std::time::Instant::now();
        let packed = pack(&input, 1 << 20);
        eprintln!(
            "{} -> {} bytes, {:?} with the check",
            input.len(),
            packed.len(),
            started.elapsed()
        );
        let name = b"SRP-crash-20261017-135202-093-p10360-t10364-0.dmp";
        let (time, date) = dos_time(2026, 10, 17, 13, 52, 3);
        let e = Entry {
            name,
            time,
            date,
            crc: crc32(0, &input),
            packed: packed.len() as u32,
            size: input.len() as u32,
        };
        let mut local = [0u8; LOCAL_MAX];
        let mut end = [0u8; TAIL_MAX];
        let n = local_header(&e, &mut local);
        let m = tail(&e, (n + packed.len()) as u32, &mut end);
        let dir = std::env::temp_dir().join(format!("srp-zip-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("test.zip");
        let mut f = std::fs::File::create(&zip).unwrap();
        f.write_all(&local[..n]).unwrap();
        f.write_all(&packed).unwrap();
        f.write_all(&end[..m]).unwrap();
        drop(f);
        let script = r#"
import sys, zipfile, zlib
with zipfile.ZipFile(sys.argv[1]) as z:
    assert z.testzip() is None
    [i] = z.infolist()
    assert i.filename == sys.argv[2] and not i.flag_bits & 0x800 and i.compress_type == 8
    assert i.date_time == (2026, 10, 17, 13, 52, 2), i.date_time
    data = z.read(i)
    assert len(data) == int(sys.argv[3]) and zlib.crc32(data) == int(sys.argv[4])
print("python zipfile: ok")
"#;
        match Command::new("python3")
            .args([
                "-I",
                "-c",
                script,
                zip.to_str().unwrap(),
                std::str::from_utf8(name).unwrap(),
                &input.len().to_string(),
                &crc32(0, &input).to_string(),
            ])
            .status()
        {
            Ok(status) => assert!(status.success()),
            Err(_) => eprintln!("no python3: zipfile not run"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
