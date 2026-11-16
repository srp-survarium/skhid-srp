//! A PNG written from a picture's rows into a [`Sink`], with no memory but
//! a [`Work`] (patch 152, docs/notes/screenshots.md).
//!
//! 24-bit colour; every row Paeth-filtered; one deflate block with one
//! Huffman code, made from a sample of the picture's rows (every
//! [`SAMPLE`]th); the only matches are runs, a byte equal to the one 1 or 3
//! before it (flat areas, most of an interface). A frame of the game comes to
//! about half its raw size, in one pass over its rows after the sample's.

#![forbid(unsafe_code)]

use crate::vostok::crc32;

/// Where the file's bytes go, in order.
pub trait Sink {
    fn write(&mut self, bytes: &[u8]);
}

/// Which of a pixel's four bytes is red.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Red, green, blue, and a byte not read.
    Rgb,
    /// Blue, green, red, and a byte not read.
    Bgr,
}

/// A picture: rows of pixels of four bytes, top to bottom.
pub trait Source {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn order(&self) -> Order;
    /// Row `y`'s pixels, `width` of them (a pixel the row does not have is
    /// black).
    fn row(&self, y: u32) -> &[u8];
}

/// The widest picture written.
pub const MAX_WIDTH: u32 = 8192;
/// A filtered row: its filter's byte and three bytes a pixel.
const ROW: usize = 1 + 3 * MAX_WIDTH as usize;
/// The most data bytes of an IDAT chunk.
const CHUNK: usize = 1 << 15;
/// A chunk in the file: its length, its type, its data, its CRC.
const CHUNK_ROOM: usize = 8 + CHUNK + 4;
/// One row in this many is read for the Huffman code.
const SAMPLE: usize = 16;

/// The memory a picture is written with: one filtered row and one chunk.
pub struct Work {
    row: [u8; ROW],
    chunk: [u8; CHUNK_ROOM],
}

impl Work {
    pub const fn new() -> Work {
        Work { row: [0; ROW], chunk: [0; CHUNK_ROOM] }
    }
}

/// A picture not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No pixels.
    Empty,
    /// Wider than [`MAX_WIDTH`].
    TooWide,
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
/// The PNG filter type of every row.
const PAETH: u8 = 4;

/// deflate's literal and length symbols: 0..=255 the bytes, 256 the block's
/// end, 257..=285 the lengths.
const SYMBOLS: usize = 286;
const END: usize = 256;
const FIRST_LENGTH: usize = 257;
/// The longest code deflate has.
const MAX_BITS: usize = 15;
/// The longest match deflate has.
const MAX_RUN: usize = 258;
/// The length symbols' first lengths and extra bits (RFC 1951, 3.2.5).
const LENGTHS: [(u16, u8); 29] = [
    (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0), (9, 0), (10, 0), (11, 1), (13, 1), (15, 1), (17, 1), (19, 2),
    (23, 2), (27, 2), (31, 2), (35, 3), (43, 3), (51, 3), (59, 3), (67, 4), (83, 4), (99, 4), (115, 4), (131, 5),
    (163, 5), (195, 5), (227, 5), (258, 0),
];
/// The distance symbols' code lengths: 0 (one byte back) and 2 (three
/// back) a bit each, 1 unused.
const DISTANCE_BITS: [u8; 3] = [1, 0, 1];

/// How far back a run's bytes repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Back {
    One,
    Three,
}

/// A step of the deflate stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Literal(u8),
    /// 3 to [`MAX_RUN`] bytes, each the byte `back` before it.
    Run { length: u16, back: Back },
}

impl Token {
    /// Its literal or length symbol, and for a run the length's extra bits
    /// (their value, how many).
    fn symbol(self) -> (usize, u32, u32) {
        match self {
            Token::Literal(byte) => (usize::from(byte), 0, 0),
            Token::Run { length, .. } => {
                let at = LENGTHS.iter().rposition(|&(first, _)| first <= length).unwrap_or(0);
                let (first, extra) = LENGTHS[at];
                (FIRST_LENGTH + at, u32::from(length - first), u32::from(extra))
            }
        }
    }
}

/// A row's bytes as tokens: the longest run from each place, a literal where
/// none is 3 long.
fn tokens(row: &[u8], mut each: impl FnMut(Token)) {
    let mut at = 0;
    while let Some(&byte) = row.get(at) {
        let run = |back: usize| match at.checked_sub(back) {
            Some(from) => row[at..].iter().zip(&row[from..]).take(MAX_RUN).take_while(|(a, b)| a == b).count(),
            None => 0,
        };
        let (one, three) = (run(1), run(3));
        let (length, back) = if one >= three { (one, Back::One) } else { (three, Back::Three) };
        if length >= 3 {
            each(Token::Run { length: length as u16, back });
            at += length;
        } else {
            each(Token::Literal(byte));
            at += 1;
        }
    }
}

/// PNG's Paeth predictor: of left, above and above-left, the nearest to
/// left + above - above-left.
fn paeth(left: u8, above: u8, above_left: u8) -> u8 {
    let (a, b, c) = (i32::from(left), i32::from(above), i32::from(above_left));
    let estimate = a + b - c;
    let (da, db, dc) = ((estimate - a).abs(), (estimate - b).abs(), (estimate - c).abs());
    if da <= db && da <= dc {
        left
    } else if db <= dc {
        above
    } else {
        above_left
    }
}

/// Row `y` filtered into `out`: the filter's byte, then red, green and blue
/// of each pixel less its prediction.
fn filter<'a>(source: &impl Source, y: u32, out: &'a mut [u8; ROW]) -> &'a [u8] {
    let width = (source.width().min(MAX_WIDTH)) as usize;
    let row = source.row(y);
    let above = if y == 0 { &[][..] } else { source.row(y - 1) };
    let places = match source.order() {
        Order::Rgb => [0, 1, 2],
        Order::Bgr => [2, 1, 0],
    };
    let (kind, pixels) = out.split_at_mut(1);
    kind[0] = PAETH;
    let (mut left, mut above_left) = ([0u8; 3], [0u8; 3]);
    let (mut row, mut above) = (row.chunks_exact(4), above.chunks_exact(4));
    for slot in pixels.chunks_exact_mut(3).take(width) {
        let (pixel, over) = (row.next(), above.next());
        for (channel, &place) in places.iter().enumerate() {
            let value = pixel.map_or(0, |p| p[place]);
            let over = over.map_or(0, |p| p[place]);
            slot[channel] = value.wrapping_sub(paeth(left[channel], over, above_left[channel]));
            left[channel] = value;
            above_left[channel] = over;
        }
    }
    &out[..1 + 3 * width]
}

/// Code lengths of at most [`MAX_BITS`] for the symbols, by how often each
/// comes (every count at least 1): a Huffman code's (Moffat and Katajainen's
/// in-place minimum redundancy), the longest codes shortened to the limit
/// (as zlib's and miniz's do).
fn code_lengths(counts: &[u32; SYMBOLS]) -> [u8; SYMBOLS] {
    const N: usize = SYMBOLS;
    // the symbols, the rarest first
    let mut order: [u16; N] = core::array::from_fn(|i| i as u16);
    order.sort_unstable_by_key(|&symbol| counts[usize::from(symbol)]);
    // a[i]: the count of the i-th rarest, then its tree's parent, then its depth
    let mut a: [u32; N] = core::array::from_fn(|i| counts[usize::from(order[i])]);
    a[0] += a[1];
    let (mut root, mut leaf) = (0, 2);
    for next in 1..N - 1 {
        if leaf >= N || a[root] < a[leaf] {
            a[next] = a[root];
            a[root] = next as u32;
            root += 1;
        } else {
            a[next] = a[leaf];
            leaf += 1;
        }
        if leaf >= N || (root < next && a[root] < a[leaf]) {
            a[next] += a[root];
            a[root] = next as u32;
            root += 1;
        } else {
            a[next] += a[leaf];
            leaf += 1;
        }
    }
    a[N - 2] = 0;
    for next in (0..N - 2).rev() {
        a[next] = a[a[next] as usize] + 1;
    }
    let (mut available, mut used, mut depth) = (1usize, 0usize, 0u32);
    let (mut root, mut next) = (N - 1, N);
    while available > 0 {
        while root > 0 && a[root - 1] == depth {
            used += 1;
            root -= 1;
        }
        while available > used {
            next -= 1;
            a[next] = depth;
            available -= 1;
        }
        available = 2 * used;
        depth += 1;
        used = 0;
    }
    // how many codes of each length; those past the limit become the limit's,
    // and codes are lengthened until the code is complete again
    let mut of_length = [0u32; 64];
    for &length in &a {
        of_length[(length as usize).min(63)] += 1;
    }
    for longer in MAX_BITS + 1..64 {
        of_length[MAX_BITS] += of_length[longer];
    }
    let mut total: u32 = (1..=MAX_BITS).map(|bits| of_length[bits] << (MAX_BITS - bits)).sum();
    while total > 1 << MAX_BITS {
        of_length[MAX_BITS] -= 1;
        if let Some(bits) = (1..MAX_BITS).rev().find(|&bits| of_length[bits] != 0) {
            of_length[bits] -= 1;
            of_length[bits + 1] += 2;
        }
        total -= 1;
    }
    // the shortest codes to the commonest symbols
    let mut lengths = [0u8; N];
    let mut rarest = order.iter().rev();
    for (bits, &count) in of_length.iter().enumerate().take(MAX_BITS + 1).skip(1) {
        for &symbol in rarest.by_ref().take(count as usize) {
            lengths[usize::from(symbol)] = bits as u8;
        }
    }
    lengths
}

/// The canonical codes of `lengths` (RFC 1951, 3.2.2), each with its bits
/// in the order they are written (the code's first bit lowest).
fn codes<const N: usize>(lengths: &[u8; N]) -> [u16; N] {
    let mut of_length = [0u16; MAX_BITS + 1];
    for &length in lengths.iter().filter(|&&length| length != 0) {
        of_length[usize::from(length)] += 1;
    }
    let mut next = [0u16; MAX_BITS + 1];
    let mut code = 0u16;
    for bits in 1..=MAX_BITS {
        code = (code + of_length[bits - 1]) << 1;
        next[bits] = code;
    }
    core::array::from_fn(|symbol| match usize::from(lengths[symbol]) {
        0 => 0,
        bits => {
            let code = next[bits];
            next[bits] += 1;
            code.reverse_bits() >> (16 - bits)
        }
    })
}

/// zlib's checksum of the bytes deflated.
struct Adler {
    low: u32,
    high: u32,
}

impl Adler {
    const MODULUS: u32 = 65521;
    /// The most bytes summed before the sums are reduced (zlib's NMAX).
    const RUN: usize = 5552;

    fn update(&mut self, bytes: &[u8]) {
        for run in bytes.chunks(Adler::RUN) {
            for &byte in run {
                self.low += u32::from(byte);
                self.high += self.low;
            }
            self.low %= Adler::MODULUS;
            self.high %= Adler::MODULUS;
        }
    }

    fn value(&self) -> u32 {
        self.high << 16 | self.low
    }
}

/// A chunk of `kind` holding `data`, in `room` (its length, type, data and
/// CRC): its bytes.
fn chunk<'a>(room: &'a mut [u8], kind: &[u8; 4], length: usize) -> &'a [u8] {
    room[..4].copy_from_slice(&(length as u32).to_be_bytes());
    room[4..8].copy_from_slice(kind);
    let crc = !crc32::update(!0, &room[4..8 + length]);
    room[8 + length..12 + length].copy_from_slice(&crc.to_be_bytes());
    &room[..12 + length]
}

/// The zlib stream going out as IDAT chunks: bits gathered into bytes, the
/// bytes into the chunk, a full chunk written.
struct Stream<'a, S: Sink> {
    sink: &'a mut S,
    chunk: &'a mut [u8; CHUNK_ROOM],
    filled: usize,
    bits: u32,
    count: u32,
}

impl<S: Sink> Stream<'_, S> {
    /// The low `count` bits of `value` (at most 16), the lowest first.
    fn bits(&mut self, value: u32, count: u32) {
        self.bits |= value << self.count;
        self.count += count;
        while self.count >= 8 {
            self.byte(self.bits as u8);
            self.bits >>= 8;
            self.count -= 8;
        }
    }

    fn byte(&mut self, byte: u8) {
        self.chunk[8 + self.filled] = byte;
        self.filled += 1;
        if self.filled == CHUNK {
            self.flush();
        }
    }

    /// The bits not yet a byte, as one (the rest of it zeros).
    fn align(&mut self) {
        if self.count > 0 {
            self.bits(0, 8 - self.count);
        }
    }

    fn flush(&mut self) {
        if self.filled > 0 {
            self.sink.write(chunk(&mut self.chunk[..], b"IDAT", self.filled));
            self.filled = 0;
        }
    }

    fn token(&mut self, token: Token, lengths: &[u8; SYMBOLS], codes: &[u16; SYMBOLS]) {
        let (symbol, extra, extra_bits) = token.symbol();
        self.bits(u32::from(codes[symbol]), u32::from(lengths[symbol]));
        if let Token::Run { back, .. } = token {
            self.bits(extra, extra_bits);
            // the distance's code: of the two, each one bit
            self.bits(u32::from(back == Back::Three), 1);
        }
    }
}

/// Writes `source` as a PNG to `sink`.
pub fn write(source: &impl Source, work: &mut Work, sink: &mut impl Sink) -> Result<(), Refusal> {
    let (width, height) = (source.width(), source.height());
    if width == 0 || height == 0 {
        return Err(Refusal::Empty);
    }
    if width > MAX_WIDTH {
        return Err(Refusal::TooWide);
    }
    let Work { row, chunk: room } = work;

    sink.write(&SIGNATURE);
    // IHDR: the size, 8 bits a sample, colour type 2 (RGB), deflate, the
    // standard filters, no interlace
    let mut header = [0u8; 8 + 13 + 4];
    header[8..12].copy_from_slice(&width.to_be_bytes());
    header[12..16].copy_from_slice(&height.to_be_bytes());
    header[16..21].copy_from_slice(&[8, 2, 0, 0, 0]);
    sink.write(chunk(&mut header, b"IHDR", 13));

    // the code, from every SAMPLEth row (and every symbol counted once, so
    // each has a code)
    let mut counts = [1u32; SYMBOLS];
    for y in (0..height).step_by(SAMPLE) {
        tokens(filter(source, y, row), |token| counts[token.symbol().0] += 1);
    }
    let lengths = code_lengths(&counts);
    let codes = codes(&lengths);

    let mut stream = Stream { sink, chunk: room, filled: 0, bits: 0, count: 0 };
    // zlib: deflate with a 32K window, no dictionary, the check bits
    stream.bits(0x78, 8);
    stream.bits(0x01, 8);
    // the last block, dynamic Huffman: 286 literal and length codes, 3
    // distance codes, 19 code length codes
    stream.bits(1, 1);
    stream.bits(2, 2);
    stream.bits(SYMBOLS as u32 - 257, 5);
    stream.bits(DISTANCE_BITS.len() as u32 - 1, 5);
    stream.bits(19 - 4, 4);
    // the code lengths' own code: 16, 17 and 18 (the repeats) unused, 0 to
    // 15 four bits each, which makes each length its own four bits
    for symbol in 0..19 {
        stream.bits(if symbol < 3 { 0 } else { 4 }, 3);
    }
    for &length in lengths.iter().chain(&DISTANCE_BITS) {
        stream.bits(u32::from(length.reverse_bits() >> 4), 4);
    }

    let mut adler = Adler { low: 1, high: 0 };
    for y in 0..height {
        let row = filter(source, y, row);
        adler.update(row);
        tokens(row, |token| stream.token(token, &lengths, &codes));
    }
    stream.bits(u32::from(codes[END]), u32::from(lengths[END]));
    stream.align();
    for byte in adler.value().to_be_bytes() {
        stream.byte(byte);
    }
    stream.flush();
    let mut end = [0u8; 12];
    stream.sink.write(chunk(&mut end, b"IEND", 0));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Image {
        width: u32,
        height: u32,
        order: Order,
        pixels: Vec<u8>,
    }

    impl Source for Image {
        fn width(&self) -> u32 {
            self.width
        }

        fn height(&self) -> u32 {
            self.height
        }

        fn order(&self) -> Order {
            self.order
        }

        fn row(&self, y: u32) -> &[u8] {
            let row = self.width as usize * 4;
            &self.pixels[y as usize * row..][..row]
        }
    }

    impl Sink for Vec<u8> {
        fn write(&mut self, bytes: &[u8]) {
            self.extend_from_slice(bytes);
        }
    }

    /// A picture like the game's: a noisy scene, a smooth sky, flat panels.
    fn image(width: u32, height: u32, order: Order) -> Image {
        let mut seed = 0x1234_5678u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (seed >> 24) as u8;
                let pixel = if y < height / 3 {
                    [(x / 2) as u8, (y * 3) as u8, 200, 255]
                } else if x > width / 2 && y > height / 2 {
                    [30, 30, 34, 255]
                } else {
                    [noise, noise / 2, (x as u8).wrapping_add(noise / 4), 0]
                };
                pixels.extend_from_slice(&pixel);
            }
        }
        Image { width, height, order, pixels }
    }

    fn encode(image: &Image) -> Vec<u8> {
        let mut work = Box::new(Work::new());
        let mut file = Vec::new();
        write(image, &mut work, &mut file).unwrap();
        file
    }

    /// The file's chunks (their types and data), every CRC checked.
    fn chunks(file: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        assert_eq!(&file[..8], &SIGNATURE);
        let (mut at, mut out) = (8, Vec::new());
        while at < file.len() {
            let length = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = file[at + 4..at + 8].try_into().unwrap();
            let data = &file[at + 8..at + 8 + length];
            let crc = u32::from_be_bytes(file[at + 8 + length..at + 12 + length].try_into().unwrap());
            assert_eq!(crc, !crc32::update(!0, &file[at + 4..at + 8 + length]), "a chunk's CRC");
            out.push((kind, data.to_vec()));
            at += 12 + length;
        }
        out
    }

    /// A reader of a deflate stream's bits.
    struct Bits<'a> {
        data: &'a [u8],
        at: usize,
    }

    impl Bits<'_> {
        fn take(&mut self, count: u32) -> u32 {
            (0..count).fold(0, |value, bit| {
                let one = self.data[self.at / 8] >> (self.at % 8) & 1;
                self.at += 1;
                value | u32::from(one) << bit
            })
        }

        /// A symbol of `code`.
        fn symbol(&mut self, code: &Code) -> usize {
            let (mut bits, mut first, mut index) = (0u32, 0u32, 0u32);
            for &count in &code.of_length[1..] {
                bits |= self.take(1);
                if bits < first + count {
                    return code.symbols[(bits - first + index) as usize];
                }
                index += count;
                first = (first + count) << 1;
                bits <<= 1;
            }
            panic!("no symbol has this code");
        }
    }

    /// A canonical code for reading: how many codes of each length, and the
    /// symbols by length, then by number.
    struct Code {
        of_length: [u32; MAX_BITS + 1],
        symbols: Vec<usize>,
    }

    impl Code {
        fn of(lengths: &[u8]) -> Code {
            let mut of_length = [0u32; MAX_BITS + 1];
            for &length in lengths.iter().filter(|&&length| length != 0) {
                of_length[usize::from(length)] += 1;
            }
            let mut symbols: Vec<usize> = (0..lengths.len()).filter(|&s| lengths[s] != 0).collect();
            symbols.sort_by_key(|&s| (lengths[s], s));
            Code { of_length, symbols }
        }
    }

    /// Inflates a zlib stream of dynamic blocks (what `write` makes; any
    /// distance), its Adler-32 checked.
    fn inflate(zlib: &[u8]) -> Vec<u8> {
        assert_eq!((u32::from(zlib[0]) << 8 | u32::from(zlib[1])) % 31, 0, "zlib's header check");
        assert_eq!(zlib[0] & 0x0F, 8, "deflate");
        let mut bits = Bits { data: &zlib[2..], at: 0 };
        let mut out = Vec::new();
        loop {
            let last = bits.take(1) == 1;
            assert_eq!(bits.take(2), 2, "a dynamic block");
            let (literals, distances, lengths) = (bits.take(5) + 257, bits.take(5) + 1, bits.take(4) + 4);
            let mut of_lengths = [0u8; 19];
            for &symbol in [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15].iter().take(lengths as usize) {
                of_lengths[symbol] = bits.take(3) as u8;
            }
            let lengths_code = Code::of(&of_lengths);
            let mut all = Vec::new();
            while all.len() < (literals + distances) as usize {
                let symbol = bits.symbol(&lengths_code);
                assert!(symbol < 16, "no repeat codes are written");
                all.push(symbol as u8);
            }
            let (literal_lengths, distance_lengths) = all.split_at(literals as usize);
            let (literal_code, distance_code) = (Code::of(literal_lengths), Code::of(distance_lengths));
            loop {
                let symbol = bits.symbol(&literal_code);
                if symbol < END {
                    out.push(symbol as u8);
                } else if symbol == END {
                    break;
                } else {
                    let (first, extra) = LENGTHS[symbol - FIRST_LENGTH];
                    let length = usize::from(first) + bits.take(u32::from(extra)) as usize;
                    let distance = match bits.symbol(&distance_code) {
                        0 => 1,
                        2 => 3,
                        other => panic!("distance symbol {other}"),
                    };
                    for _ in 0..length {
                        out.push(out[out.len() - distance]);
                    }
                }
            }
            if last {
                break;
            }
        }
        let end = 2 + bits.at.div_ceil(8);
        let mut adler = Adler { low: 1, high: 0 };
        adler.update(&out);
        assert_eq!(&zlib[end..], &adler.value().to_be_bytes(), "the Adler-32, the stream's last bytes");
        out
    }

    /// The file decoded: its size and its pixels, red, green and blue each.
    fn decode(file: &[u8]) -> (u32, u32, Vec<u8>) {
        let chunks = chunks(file);
        let kinds: Vec<&[u8; 4]> = chunks.iter().map(|(kind, _)| kind).collect();
        assert_eq!(kinds.first(), Some(&b"IHDR"));
        assert_eq!(kinds.last(), Some(&b"IEND"));
        assert!(kinds[1..kinds.len() - 1].iter().all(|kind| *kind == b"IDAT"));
        let header = &chunks[0].1;
        let width = u32::from_be_bytes(header[..4].try_into().unwrap());
        let height = u32::from_be_bytes(header[4..8].try_into().unwrap());
        assert_eq!(&header[8..], &[8, 2, 0, 0, 0], "8 bits, RGB, no interlace");
        let zlib: Vec<u8> = chunks[1..chunks.len() - 1].iter().flat_map(|(_, data)| data.clone()).collect();
        let filtered = inflate(&zlib);
        let stride = 1 + 3 * width as usize;
        assert_eq!(filtered.len(), stride * height as usize);
        let mut pixels: Vec<u8> = Vec::new();
        for (y, row) in filtered.chunks(stride).enumerate() {
            assert_eq!(row[0], PAETH);
            for (x, &byte) in row[1..].iter().enumerate() {
                let above = |back: usize| if y > 0 && x >= back { pixels[(y - 1) * (stride - 1) + x - back] } else { 0 };
                let left = if x >= 3 { pixels[y * (stride - 1) + x - 3] } else { 0 };
                pixels.push(byte.wrapping_add(paeth(left, above(0), above(3))));
            }
        }
        (width, height, pixels)
    }

    fn rgb(image: &Image) -> Vec<u8> {
        image
            .pixels
            .chunks(4)
            .flat_map(|p| match image.order {
                Order::Rgb => [p[0], p[1], p[2]],
                Order::Bgr => [p[2], p[1], p[0]],
            })
            .collect()
    }

    #[test]
    fn a_picture_comes_back_pixel_for_pixel() {
        for (width, height, order) in [(320, 200, Order::Rgb), (257, 33, Order::Bgr), (1, 1, Order::Rgb), (3, 700, Order::Bgr)] {
            let image = image(width, height, order);
            let file = encode(&image);
            let (w, h, pixels) = decode(&file);
            assert_eq!((w, h), (width, height));
            assert!(pixels == rgb(&image), "{width} x {height}: the pixels differ");
        }
    }

    #[test]
    fn a_large_picture_takes_several_chunks_and_is_smaller_than_raw() {
        let image = image(640, 400, Order::Rgb);
        let file = encode(&image);
        let chunks = chunks(&file);
        assert!(chunks.len() > 4, "{} chunks", chunks.len());
        assert!(chunks.iter().all(|(_, data)| data.len() <= CHUNK));
        assert!(file.len() < 640 * 400 * 3 * 2 / 3, "{} bytes of {}", file.len(), 640 * 400 * 3);
        assert!(decode(&file).2 == rgb(&image));
    }

    #[test]
    fn flat_and_run_heavy_rows_are_runs() {
        let mut found = Vec::new();
        tokens(&[4, 0, 0, 0, 0, 0, 7, 1, 2, 3, 1, 2, 3, 1, 2, 9], |token| found.push(token));
        assert_eq!(
            found,
            [
                Token::Literal(4),
                Token::Literal(0),
                Token::Run { length: 4, back: Back::One },
                Token::Literal(7),
                Token::Literal(1),
                Token::Literal(2),
                Token::Literal(3),
                Token::Run { length: 5, back: Back::Three },
                Token::Literal(9),
            ]
        );
        // a run is cut at deflate's longest match
        let mut runs = Vec::new();
        tokens(&[0; 600], |token| runs.push(token));
        assert_eq!(
            runs,
            [
                Token::Literal(0),
                Token::Run { length: 258, back: Back::One },
                Token::Run { length: 258, back: Back::One },
                Token::Run { length: 83, back: Back::One },
            ]
        );
    }

    #[test]
    fn length_symbols_and_extra_bits() {
        let of = |length| Token::Run { length, back: Back::One }.symbol();
        assert_eq!(of(3), (257, 0, 0));
        assert_eq!(of(10), (264, 0, 0));
        assert_eq!(of(12), (265, 1, 1));
        assert_eq!(of(257), (284, 30, 5));
        assert_eq!(of(258), (285, 0, 0));
    }

    fn kraft(lengths: &[u8]) -> u32 {
        lengths.iter().filter(|&&l| l != 0).map(|&l| 1u32 << (MAX_BITS - usize::from(l))).sum()
    }

    #[test]
    fn code_lengths_are_complete_and_within_the_limit() {
        // even counts: nine bits for most
        let even = code_lengths(&[7; SYMBOLS]);
        assert_eq!(kraft(&even), 1 << MAX_BITS);
        assert!(even.iter().all(|&l| l == 8 || l == 9));
        // counts that double
        let mut counts = [1u32; SYMBOLS];
        for (i, count) in counts.iter_mut().enumerate() {
            *count = 1 << (i % 16);
        }
        let steep = code_lengths(&counts);
        assert_eq!(kraft(&steep), 1 << MAX_BITS);
        assert!(steep.iter().all(|&l| (1..=MAX_BITS as u8).contains(&l)));
        // Fibonacci counts: the deepest code a total can make
        let (mut a, mut b) = (1u32, 1u32);
        for count in counts.iter_mut().take(40) {
            *count = a;
            (a, b) = (b, a + b);
        }
        let deep = code_lengths(&counts);
        assert_eq!(kraft(&deep), 1 << MAX_BITS);
        assert!(deep.iter().all(|&l| (1..=MAX_BITS as u8).contains(&l)));
        // the commonest symbol has the shortest code
        let mut skewed = [1u32; SYMBOLS];
        skewed[0] = 100_000;
        let lengths = code_lengths(&skewed);
        assert_eq!(lengths[0], 1);
    }

    #[test]
    fn canonical_codes_are_in_written_order() {
        // RFC 1951's example: lengths 3,3,3,3,3,2,4,4 give 010,011,100,101,110,00,1110,1111
        let codes = codes(&[3u8, 3, 3, 3, 3, 2, 4, 4]);
        assert_eq!(codes, [0b010, 0b110, 0b001, 0b101, 0b011, 0b00, 0b0111, 0b1111]);
    }

    #[test]
    fn pictures_not_written() {
        let mut work = Box::new(Work::new());
        let mut file = Vec::new();
        let wide = Image { width: MAX_WIDTH + 1, height: 1, order: Order::Rgb, pixels: Vec::new() };
        assert_eq!(write(&wide, &mut work, &mut file), Err(Refusal::TooWide));
        let none = Image { width: 0, height: 10, order: Order::Rgb, pixels: Vec::new() };
        assert_eq!(write(&none, &mut work, &mut file), Err(Refusal::Empty));
        assert!(file.is_empty());
    }

    #[test]
    fn adler_of_a_known_text() {
        let mut adler = Adler { low: 1, high: 0 };
        adler.update(b"Wikipedia");
        assert_eq!(adler.value(), 0x11E6_0398);
    }
}
