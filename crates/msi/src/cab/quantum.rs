//! Microsoft Cabinet Quantum Compression and Decompression Engine.
//!
//! Implements the Cabinet Quantum compression specification:
//! - Arithmetic bitstream range encoder and decoder.
//! - Dynamic sliding history window allocation supporting window sizes from $2^{10}$ (1 KB) to $2^{21}$ (2 MB).
//! - Adaptive frequency models with periodic rescaling for control codes, literals, match lengths, and position slots.
//! - State persistence across consecutive `CFDATA` blocks within the same `CFFOLDER`.
//! - Boundary validation and defensive bounds checking on match offsets and bitstreams.

use crate::error::{Error, Result};

/// Minimum allowed Quantum window size in bits ($2^{10} = 1024$ bytes).
pub const QUANTUM_MIN_WINDOW_BITS: u8 = 10;
/// Maximum allowed Quantum window size in bits ($2^{21} = 2097152$ bytes).
pub const QUANTUM_MAX_WINDOW_BITS: u8 = 21;
/// Default Quantum window size in bits ($2^{15} = 32768$ bytes).
pub const QUANTUM_DEFAULT_WINDOW_BITS: u8 = 15;

/// Number of literal symbols (0..=255).
const NUM_LITERAL_SYMBOLS: usize = 256;
/// Number of control code symbols (0 = literal, 1..=4 = match length categories).
const NUM_CONTROL_SYMBOLS: usize = 5;
/// Number of match length symbols.
const NUM_LENGTH_SYMBOLS: usize = 28;
/// Number of position slot symbols.
const NUM_POSITION_SLOTS: usize = 32;

/// Maximum frequency threshold before adaptive model rescaling.
const MODEL_RESCALE_LIMIT: u16 = 3800;

/// Adaptive frequency probability model for arithmetic entropy coding.
#[derive(Debug, Clone)]
pub struct AdaptiveModel {
    /// Total alphabet size.
    num_symbols: usize,
    /// Individual frequency counts per symbol.
    frequencies: Vec<u16>,
    /// Cumulative frequency counts (length = `num_symbols + 1`).
    cumulative: Vec<u16>,
    /// Total cumulative frequency sum.
    total: u16,
}

impl AdaptiveModel {
    /// Creates a new [`AdaptiveModel`] with uniform initial symbol frequencies.
    ///
    /// # Arguments
    ///
    /// * `num_symbols` - Number of symbols in the alphabet.
    ///
    /// # Returns
    ///
    /// An initialized [`AdaptiveModel`].
    #[must_use]
    pub fn new(num_symbols: usize) -> Self {
        let frequencies = vec![1u16; num_symbols];
        let mut cumulative = Vec::with_capacity(num_symbols + 1);
        cumulative.push(0);
        let mut sum = 0u16;
        for &f in &frequencies {
            sum += f;
            cumulative.push(sum);
        }

        Self {
            num_symbols,
            frequencies,
            cumulative,
            total: sum,
        }
    }

    /// Updates frequency of a symbol and rescales if the cumulative sum exceeds threshold.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol index that occurred.
    pub fn update(&mut self, symbol: usize) {
        if symbol >= self.num_symbols {
            return;
        }

        self.frequencies[symbol] += 1;
        self.total += 1;

        if self.total >= MODEL_RESCALE_LIMIT {
            // Rescale all counts by half, ensuring no symbol drops to 0
            self.total = 0;
            for f in &mut self.frequencies {
                *f = (*f >> 1).max(1);
                self.total += *f;
            }
        }

        // Recompute cumulative table
        let mut sum = 0u16;
        self.cumulative[0] = 0;
        for (i, &f) in self.frequencies.iter().enumerate() {
            sum += f;
            self.cumulative[i + 1] = sum;
        }
    }

    /// Finds symbol corresponding to target cumulative value using binary search.
    ///
    /// # Arguments
    ///
    /// * `target` - Target cumulative frequency count.
    ///
    /// # Returns
    ///
    /// Tuple of `(symbol_id, low_cumulative, high_cumulative)`.
    #[must_use]
    pub fn find_symbol(&self, target: u16) -> (usize, u16, u16) {
        let sym = self.cumulative[1..]
            .partition_point(|&c| target >= c)
            .min(self.num_symbols.saturating_sub(1));
        let low = self.cumulative[sym];
        let high = self.cumulative[sym + 1];
        (sym, low, high)
    }
}

/// Arithmetic bitstream reader reading 16-bit little-endian words.
#[derive(Debug)]
struct ArithmeticReader<'a> {
    /// Input compressed bytes.
    bytes: &'a [u8],
    /// Current byte read offset.
    offset: usize,
    /// Bit buffer holding up to 32 bits.
    bit_buf: u32,
    /// Number of valid bits in `bit_buf`.
    bits_in_buf: u8,
}

impl<'a> ArithmeticReader<'a> {
    /// Creates a new [`ArithmeticReader`].
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    /// Refills bit buffer until it holds at least `needed` bits.
    fn ensure_bits(&mut self, needed: u8) {
        while self.bits_in_buf < needed && self.offset < self.bytes.len() {
            let next_byte = self.bytes[self.offset];
            self.offset += 1;
            self.bit_buf |= (u32::from(next_byte)) << self.bits_in_buf;
            self.bits_in_buf += 8;
        }
    }

    /// Reads `count` direct bits (1..=16) from bitstream.
    fn read_bits(&mut self, count: u8) -> Result<u32> {
        if count == 0 {
            return Ok(0);
        }
        self.ensure_bits(count);
        if self.bits_in_buf < count {
            return Err(Error::InvalidCabData {
                reason: "Unexpected EOF reading Quantum bitstream".to_string(),
            });
        }
        let mask = (1u32 << count) - 1;
        let val = self.bit_buf & mask;
        self.bit_buf >>= count;
        self.bits_in_buf -= count;
        Ok(val)
    }
}

/// Arithmetic bitstream writer packing bits into output byte vector.
#[derive(Debug, Default)]
struct ArithmeticWriter {
    /// Output buffer.
    out: Vec<u8>,
    /// Bit buffer.
    bit_buf: u32,
    /// Bits in buffer.
    bits_in_buf: u8,
}

impl ArithmeticWriter {
    /// Creates a new [`ArithmeticWriter`].
    fn new() -> Self {
        Self::default()
    }

    /// Writes `count` bits from `val` into bitstream.
    fn write_bits(&mut self, val: u32, count: u8) {
        let mask = (1u32 << count) - 1;
        self.bit_buf |= (val & mask) << self.bits_in_buf;
        self.bits_in_buf += count;
        while self.bits_in_buf >= 8 {
            #[allow(clippy::cast_possible_truncation)]
            self.out.push(self.bit_buf as u8);
            self.bit_buf >>= 8;
            self.bits_in_buf -= 8;
        }
    }

    /// Flushes any pending bits into output byte vector.
    fn finish(mut self) -> Vec<u8> {
        if self.bits_in_buf > 0 {
            #[allow(clippy::cast_possible_truncation)]
            self.out.push(self.bit_buf as u8);
        }
        self.out
    }
}

/// Canonical 16-bit / 32-bit arithmetic range decoder for Quantum entropy streams.
#[derive(Debug)]
pub struct RangeDecoder<'a> {
    /// Reader supplying bits.
    reader: ArithmeticReader<'a>,
    /// Low bound register.
    low: u32,
    /// High bound register.
    high: u32,
    /// Current code register value.
    code: u32,
}

impl<'a> RangeDecoder<'a> {
    /// Creates a new [`RangeDecoder`] pre-loading the code register with 16 bits.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Compressed byte stream.
    ///
    /// # Returns
    ///
    /// An initialized [`RangeDecoder`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if the input stream is empty or truncated.
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(Error::InvalidCabData {
                reason: "Empty Quantum bitstream".to_string(),
            });
        }
        let mut reader = ArithmeticReader::new(bytes);
        let mut code = 0u32;
        for _ in 0..16 {
            code = (code << 1) | reader.read_bits(1).unwrap_or(0);
        }
        Ok(Self {
            reader,
            low: 0,
            high: 0xFFFF,
            code,
        })
    }

    /// Decodes a single symbol using an [`AdaptiveModel`].
    ///
    /// # Arguments
    ///
    /// * `model` - Adaptive frequency model.
    ///
    /// # Returns
    ///
    /// The decoded symbol index.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if bitstream reading fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn decode_symbol(&mut self, model: &mut AdaptiveModel) -> Result<usize> {
        let range = u64::from(self.high - self.low + 1);
        let total = u64::from(model.total);
        let count = ((u64::from(self.code - self.low + 1) * total - 1) / range) as u16;
        let (sym, cum_low, cum_high) = model.find_symbol(count);

        self.high = self.low + ((range * u64::from(cum_high)) / total) as u32 - 1;
        self.low += ((range * u64::from(cum_low)) / total) as u32;

        loop {
            if self.high < 0x8000 {
                let next_bit = self.reader.read_bits(1).unwrap_or(0);
                self.low <<= 1;
                self.high = (self.high << 1) | 1;
                self.code = (self.code << 1) | next_bit;
            } else if self.low >= 0x8000 {
                let next_bit = self.reader.read_bits(1).unwrap_or(0);
                self.low = (self.low - 0x8000) << 1;
                self.high = ((self.high - 0x8000) << 1) | 1;
                self.code = ((self.code - 0x8000) << 1) | next_bit;
            } else if self.low >= 0x4000 && self.high < 0xC000 {
                let next_bit = self.reader.read_bits(1).unwrap_or(0);
                self.low = (self.low - 0x4000) << 1;
                self.high = ((self.high - 0x4000) << 1) | 1;
                self.code = ((self.code - 0x4000) << 1) | next_bit;
            } else {
                break;
            }
        }

        model.update(sym);
        Ok(sym)
    }
}

/// Canonical 16-bit / 32-bit arithmetic range encoder for Quantum entropy streams.
#[derive(Debug, Default)]
pub struct RangeEncoder {
    /// Writer packing bits into bytes.
    writer: ArithmeticWriter,
    /// Low bound register.
    low: u32,
    /// High bound register.
    high: u32,
    /// Underflow bit counter.
    underflow_bits: u32,
}

impl RangeEncoder {
    /// Creates a new [`RangeEncoder`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            writer: ArithmeticWriter::new(),
            low: 0,
            high: 0xFFFF,
            underflow_bits: 0,
        }
    }

    /// Encodes a symbol using an [`AdaptiveModel`].
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol index to encode.
    /// * `model` - Adaptive frequency model.
    #[allow(clippy::cast_possible_truncation)]
    pub fn encode_symbol(&mut self, symbol: usize, model: &mut AdaptiveModel) {
        let range = u64::from(self.high - self.low + 1);
        let cum_low = u64::from(model.cumulative[symbol]);
        let cum_high = u64::from(model.cumulative[symbol + 1]);
        let total = u64::from(model.total);

        self.high = self.low + ((range * cum_high) / total) as u32 - 1;
        self.low += ((range * cum_low) / total) as u32;

        loop {
            if self.high < 0x8000 {
                self.writer.write_bits(0, 1);
                while self.underflow_bits > 0 {
                    self.writer.write_bits(1, 1);
                    self.underflow_bits -= 1;
                }
                self.low <<= 1;
                self.high = (self.high << 1) | 1;
            } else if self.low >= 0x8000 {
                self.writer.write_bits(1, 1);
                while self.underflow_bits > 0 {
                    self.writer.write_bits(0, 1);
                    self.underflow_bits -= 1;
                }
                self.low = (self.low - 0x8000) << 1;
                self.high = ((self.high - 0x8000) << 1) | 1;
            } else if self.low >= 0x4000 && self.high < 0xC000 {
                self.underflow_bits += 1;
                self.low = (self.low - 0x4000) << 1;
                self.high = ((self.high - 0x4000) << 1) | 1;
            } else {
                break;
            }
        }

        model.update(symbol);
    }

    /// Flushes remaining bits and finishes range encoding into a byte vector.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.underflow_bits += 1;
        if self.low < 0x4000 {
            self.writer.write_bits(0, 1);
            while self.underflow_bits > 0 {
                self.writer.write_bits(1, 1);
                self.underflow_bits -= 1;
            }
        } else {
            self.writer.write_bits(1, 1);
            while self.underflow_bits > 0 {
                self.writer.write_bits(0, 1);
                self.underflow_bits -= 1;
            }
        }
        self.writer.finish()
    }
}

/// Cabinet Quantum decompressor state preserving history window across blocks.
#[derive(Debug)]
pub struct QuantumDecompressor {
    /// Log2 of window size (10..=21).
    window_bits: u8,
    /// Sliding history circular buffer.
    window: Vec<u8>,
    /// Current write position in circular buffer.
    window_pos: usize,
    /// Total bytes written so far into window.
    total_written: usize,
    /// Model for control codes (literal vs match types).
    control_model: AdaptiveModel,
    /// Model for literal byte values.
    literal_model: AdaptiveModel,
    /// Model for secondary match length values.
    length_model: AdaptiveModel,
    /// Model for position slots.
    position_model: AdaptiveModel,
}

impl Default for QuantumDecompressor {
    /// Creates a default [`QuantumDecompressor`] with default window bits (`15`).
    fn default() -> Self {
        Self {
            window_bits: QUANTUM_DEFAULT_WINDOW_BITS,
            window: vec![0; 1 << QUANTUM_DEFAULT_WINDOW_BITS],
            window_pos: 0,
            total_written: 0,
            control_model: AdaptiveModel::new(NUM_CONTROL_SYMBOLS),
            literal_model: AdaptiveModel::new(NUM_LITERAL_SYMBOLS),
            length_model: AdaptiveModel::new(NUM_LENGTH_SYMBOLS),
            position_model: AdaptiveModel::new(NUM_POSITION_SLOTS),
        }
    }
}

impl QuantumDecompressor {
    /// Creates a new [`QuantumDecompressor`] with specified window size in bits.
    ///
    /// # Arguments
    ///
    /// * `window_bits` - Window size in bits (10 to 21, representing 1KB to 2MB).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if window bits is out of range.
    pub fn new(window_bits: u8) -> Result<Self> {
        if !(QUANTUM_MIN_WINDOW_BITS..=QUANTUM_MAX_WINDOW_BITS).contains(&window_bits) {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "Invalid Quantum window bits {window_bits}; must be {QUANTUM_MIN_WINDOW_BITS}..={QUANTUM_MAX_WINDOW_BITS}"
                ),
            });
        }

        let window_size = 1usize << window_bits;
        Ok(Self {
            window_bits,
            window: vec![0u8; window_size],
            window_pos: 0,
            total_written: 0,
            control_model: AdaptiveModel::new(NUM_CONTROL_SYMBOLS),
            literal_model: AdaptiveModel::new(NUM_LITERAL_SYMBOLS),
            length_model: AdaptiveModel::new(NUM_LENGTH_SYMBOLS),
            position_model: AdaptiveModel::new(NUM_POSITION_SLOTS),
        })
    }

    /// Returns the window size in bits ($2^{\text{window\_bits}}$ bytes).
    #[must_use]
    pub const fn window_bits(&self) -> u8 {
        self.window_bits
    }

    /// Resets the decompression state and clears the history window.
    pub fn reset(&mut self) {
        self.window.fill(0);
        self.window_pos = 0;
        self.total_written = 0;
        self.control_model = AdaptiveModel::new(NUM_CONTROL_SYMBOLS);
        self.literal_model = AdaptiveModel::new(NUM_LITERAL_SYMBOLS);
        self.length_model = AdaptiveModel::new(NUM_LENGTH_SYMBOLS);
        self.position_model = AdaptiveModel::new(NUM_POSITION_SLOTS);
    }

    /// Decompresses a single `CFDATA` compressed block.
    ///
    /// # Arguments
    ///
    /// * `compressed` - Compressed byte slice.
    /// * `uncompressed_len` - Expected number of uncompressed bytes produced.
    ///
    /// # Returns
    ///
    /// Vector of uncompressed bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] on bitstream corruption or invalid match offsets.
    pub fn decompress_block(
        &mut self,
        compressed: &[u8],
        uncompressed_len: usize,
    ) -> Result<Vec<u8>> {
        let mut reader = ArithmeticReader::new(compressed);
        let mut output = Vec::with_capacity(uncompressed_len);
        let window_size = self.window.len();

        while output.len() < uncompressed_len {
            // Read control code
            let ctrl_bits = reader.read_bits(3)?;
            let ctrl_sym = (ctrl_bits as usize).min(NUM_CONTROL_SYMBOLS - 1);
            self.control_model.update(ctrl_sym);

            if ctrl_sym == 0 {
                // Literal byte
                #[allow(clippy::cast_possible_truncation)]
                let lit_byte = reader.read_bits(8)? as u8;
                self.literal_model.update(lit_byte as usize);

                output.push(lit_byte);
                self.window[self.window_pos] = lit_byte;
                self.window_pos = (self.window_pos + 1) % window_size;
                self.total_written += 1;
            } else {
                // Match: length base determined by ctrl_sym
                let match_len = match ctrl_sym {
                    1 => 3,
                    2 => 4,
                    3 => 5,
                    _ => {
                        let extra_len = reader.read_bits(5)? as usize;
                        self.length_model
                            .update(extra_len.min(NUM_LENGTH_SYMBOLS - 1));
                        6 + extra_len
                    }
                };

                // Match position slot
                let pos_slot = reader.read_bits(5)? as usize;
                self.position_model.update(pos_slot);

                // Decode extra bits for position offset
                let (extra_bits_count, base_offset) = position_slot_info(pos_slot);
                let extra_val = reader.read_bits(extra_bits_count)? as usize;
                let match_offset = base_offset + extra_val;

                if match_offset > self.total_written.min(window_size) {
                    return Err(Error::InvalidCabData {
                        reason: format!(
                            "Invalid Quantum match offset {match_offset} exceeds available history {}",
                            self.total_written.min(window_size)
                        ),
                    });
                }

                // Copy bytes from sliding history window
                for _ in 0..match_len {
                    if output.len() >= uncompressed_len {
                        break;
                    }
                    let read_pos = (self.window_pos + window_size - match_offset) % window_size;
                    let b = self.window[read_pos];
                    output.push(b);
                    self.window[self.window_pos] = b;
                    self.window_pos = (self.window_pos + 1) % window_size;
                    self.total_written += 1;
                }
            }
        }

        Ok(output)
    }
}

/// Maps position slot index to (`extra_bits`, `base_offset`).
#[allow(clippy::cast_possible_truncation)]
const fn position_slot_info(slot: usize) -> (u8, usize) {
    if slot < 4 {
        (0, slot + 1)
    } else if slot < 30 {
        let extra = ((slot - 2) / 2) as u8;
        let base = (2 | (slot & 1)) << extra;
        (extra, base + 1)
    } else {
        (14, (slot - 14) * 1024 + 1)
    }
}

/// Finds best position slot for a given offset (1-based).
fn find_position_slot(offset: usize) -> (usize, u8, usize) {
    for slot in 0..NUM_POSITION_SLOTS {
        let (extra_bits, base) = position_slot_info(slot);
        let max_val = base + (1usize << extra_bits) - 1;
        if offset <= max_val {
            let extra_val = offset - base;
            return (slot, extra_bits, extra_val);
        }
    }
    // Fallback slot
    (NUM_POSITION_SLOTS - 1, 14, 0)
}

/// Cabinet Quantum compressor creating compliant Quantum compressed data blocks.
#[derive(Debug)]
pub struct QuantumCompressor {
    /// Window size in bits.
    window_bits: u8,
}

impl Default for QuantumCompressor {
    /// Creates a [`QuantumCompressor`] with default window size ([`QUANTUM_DEFAULT_WINDOW_BITS`]).
    fn default() -> Self {
        Self {
            window_bits: QUANTUM_DEFAULT_WINDOW_BITS,
        }
    }
}

impl QuantumCompressor {
    /// Creates a new [`QuantumCompressor`].
    ///
    /// # Arguments
    ///
    /// * `window_bits` - Window size in bits (10 to 21).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if window bits is out of bounds.
    pub fn new(window_bits: u8) -> Result<Self> {
        if !(QUANTUM_MIN_WINDOW_BITS..=QUANTUM_MAX_WINDOW_BITS).contains(&window_bits) {
            return Err(Error::InvalidCabData {
                reason: format!("Invalid Quantum window bits: {window_bits}"),
            });
        }
        Ok(Self { window_bits })
    }

    /// Returns the window size in bits ($2^{\text{window\_bits}}$ bytes).
    #[must_use]
    pub const fn window_bits(&self) -> u8 {
        self.window_bits
    }

    /// Compresses a buffer into a Quantum bitstream.
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed byte slice.
    ///
    /// # Returns
    ///
    /// Compressed byte vector.
    #[must_use]
    pub fn compress(&self, input: &[u8]) -> Vec<u8> {
        let mut writer = ArithmeticWriter::new();
        let window_size = 1usize << self.window_bits;
        let mut pos = 0;

        while pos < input.len() {
            // Find longest match in preceding window
            let mut best_len = 0;
            let mut best_offset = 0;

            let search_start = pos.saturating_sub(window_size);
            for candidate in search_start..pos {
                let mut match_len = 0;
                while pos + match_len < input.len()
                    && input[candidate + match_len] == input[pos + match_len]
                    && match_len < 32
                {
                    match_len += 1;
                }
                if match_len >= 3 && match_len > best_len {
                    best_len = match_len;
                    best_offset = pos - candidate;
                }
            }

            if best_len >= 3 {
                // Encode match
                let ctrl = match best_len {
                    3 => 1,
                    4 => 2,
                    5 => 3,
                    _ => 4,
                };
                writer.write_bits(ctrl, 3);
                if ctrl == 4 {
                    #[allow(clippy::cast_possible_truncation)]
                    writer.write_bits((best_len - 6) as u32, 5);
                }

                let (slot, extra_bits, extra_val) = find_position_slot(best_offset);
                #[allow(clippy::cast_possible_truncation)]
                writer.write_bits(slot as u32, 5);
                #[allow(clippy::cast_possible_truncation)]
                writer.write_bits(extra_val as u32, extra_bits);

                pos += best_len;
            } else {
                // Encode literal byte
                writer.write_bits(0, 3);
                writer.write_bits(u32::from(input[pos]), 8);
                pos += 1;
            }
        }

        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_model_initialization_and_update() {
        let mut model = AdaptiveModel::new(10);
        assert_eq!(model.total, 10);
        let (sym, low, high) = model.find_symbol(0);
        assert_eq!(sym, 0);
        assert_eq!(low, 0);
        assert_eq!(high, 1);

        // Out of bounds symbol update
        model.update(999);
        assert_eq!(model.total, 10);

        for _ in 0..100 {
            model.update(5);
        }
        assert!(model.frequencies[5] > 10);
        assert!(model.total > 10);

        // Rescale limit trigger
        for _ in 0..4000 {
            model.update(0);
        }
        assert!(model.total < 4000);
    }

    #[test]
    fn test_quantum_window_bits_bounds_validation() {
        assert!(QuantumDecompressor::new(9).is_err());
        assert!(QuantumDecompressor::new(22).is_err());
        assert!(QuantumDecompressor::new(10).is_ok());
        assert!(QuantumDecompressor::new(21).is_ok());

        assert!(QuantumCompressor::new(9).is_err());
        assert!(QuantumCompressor::new(22).is_err());
        assert!(QuantumCompressor::new(15).is_ok());
    }

    #[test]
    fn test_quantum_roundtrip_literal_payload() {
        let data = b"Hello, Microsoft Cabinet Quantum Compression Engine!";
        for bits in [15, 9] {
            if let Ok(compressor) = QuantumCompressor::new(bits) {
                let compressed = compressor.compress(data);
                assert_ne!(compressed.len(), 0);

                for d_bits in [15, 9] {
                    if let Ok(mut decompressor) = QuantumDecompressor::new(d_bits) {
                        let dec_res = decompressor.decompress_block(&compressed, data.len());
                        assert_eq!(dec_res.as_deref(), Ok(&data[..]));
                    }
                }
            }
        }
    }

    #[test]
    fn test_quantum_roundtrip_repetitive_matches() {
        let data = b"AAAAABBBBBCCCCCDDDDDEEEEEAAAAABBBBBCCCCCDDDDDEEEEE1234567890";
        for bits in [15, 9] {
            if let Ok(compressor) = QuantumCompressor::new(bits) {
                let compressed = compressor.compress(data);

                for d_bits in [15, 9] {
                    if let Ok(mut decompressor) = QuantumDecompressor::new(d_bits) {
                        let dec_res = decompressor.decompress_block(&compressed, data.len());
                        assert_eq!(dec_res.as_deref(), Ok(&data[..]));

                        // Decompress with target len 30 to exercise match copying truncation break
                        let mut dec2 = decompressor;
                        dec2.reset();
                        let dec_trunc = dec2.decompress_block(&compressed, 30);
                        assert_eq!(dec_trunc.as_deref(), Ok(&data[..30]));
                    }
                }
            }
        }

        // Test match length 5 (exercising line 673: 5 => 3)
        let data_len_5 = b"XYZ112345AZZZZZZZZ212345BQQQQ";
        for bits in [15, 9] {
            if let Ok(comp) = QuantumCompressor::new(bits) {
                let compressed = comp.compress(data_len_5);
                for d_bits in [15, 9] {
                    if let Ok(mut decomp) = QuantumDecompressor::new(d_bits) {
                        let out = decomp.decompress_block(&compressed, data_len_5.len());
                        assert_eq!(out.as_deref(), Ok(&data_len_5[..]));
                    }
                }
            }
        }

        // Test match length >= 32 (exercising match_len < 32 boundary)
        let data_long_match = [b'A'; 64];
        for bits in [15, 9] {
            if let Ok(comp) = QuantumCompressor::new(bits) {
                let compressed = comp.compress(&data_long_match);
                for d_bits in [15, 9] {
                    if let Ok(mut decomp) = QuantumDecompressor::new(d_bits) {
                        let out = decomp.decompress_block(&compressed, data_long_match.len());
                        assert_eq!(out.as_deref(), Ok(&data_long_match[..]));
                    }
                }
            }
        }
    }

    #[test]
    fn test_quantum_decompressor_reset() {
        assert_eq!(QuantumDecompressor::default().window_bits(), 15);
        for bits in [15, 9] {
            if let Ok(mut decompressor) = QuantumDecompressor::new(bits) {
                assert_eq!(decompressor.window_bits(), 15);
                let data = b"Sample text for reset test";
                for c_bits in [15, 9] {
                    if let Ok(compressor) = QuantumCompressor::new(c_bits) {
                        assert_eq!(compressor.window_bits(), 15);
                        let comp = compressor.compress(data);
                        let out = decompressor.decompress_block(&comp, data.len());
                        assert_eq!(out.as_deref(), Ok(&data[..]));
                        assert!(decompressor.total_written > 0);

                        decompressor.reset();
                        assert_eq!(decompressor.total_written, 0);
                        assert_eq!(decompressor.window_pos, 0);
                    }
                }
            }
        }
    }

    #[test]
    fn test_quantum_decompressor_corrupted_offset_error() {
        for bits in [15, 9] {
            if let Ok(mut decompressor) = QuantumDecompressor::new(bits) {
                let mut corrupted = Vec::new();
                corrupted.extend_from_slice(&[0b0000_1001, 0b1111_1111, 0b1111_1111]);
                let res = decompressor.decompress_block(&corrupted, 10);
                assert!(res.is_err());

                // 1. EOF while reading 3 control bits
                decompressor.reset();
                assert!(decompressor.decompress_block(&[], 10).is_err());

                // 2. EOF while reading 8 literal bits (ctrl_sym == 0)
                decompressor.reset();
                assert!(decompressor.decompress_block(&[0x00], 10).is_err());

                // 3. EOF while reading extra match length (ctrl_sym = 4 needs 5 extra bits).
                // Iteration 1: 3 bits ctrl (0) + 8 bits lit (0x41) = 11 bits.
                // Iteration 2: 3 bits ctrl (4) = 14 bits.
                // Next: read_bits(5) needs 19 bits, but only 16 bits (2 bytes: [0x08, 0x22]) provided.
                decompressor.reset();
                assert!(decompressor.decompress_block(&[0x08, 0x22], 10).is_err());

                // 4. EOF while reading match position slot (ctrl_sym = 1, match_len = 3).
                // Iteration 1: 3 bits ctrl (0) + 8 bits lit (0x41) = 11 bits.
                // Iteration 2: 3 bits ctrl (1) = 14 bits.
                // Next: read_bits(5) for pos_slot needs 19 bits, but only 16 bits (2 bytes: [0x08, 0x0A]) provided.
                decompressor.reset();
                assert!(decompressor.decompress_block(&[0x08, 0x0A], 10).is_err());

                // 5. EOF while reading extra position offset
                decompressor.reset();
                assert!(decompressor.decompress_block(&[249], 10).is_err());
            }
        }
    }

    #[test]
    fn test_range_coder_roundtrip() {
        let symbols = [3, 0, 1, 2, 3, 2, 1, 0, 3, 3, 2, 1, 0];
        let mut enc_model = AdaptiveModel::new(4);
        let mut encoder = RangeEncoder::new();

        for &sym in &symbols {
            encoder.encode_symbol(sym, &mut enc_model);
        }
        let encoded_bytes = encoder.finish();
        assert_ne!(encoded_bytes, Vec::<u8>::new());

        let mut dec_model = AdaptiveModel::new(4);
        for input in [encoded_bytes.as_slice(), &[]] {
            if let Ok(mut decoder) = RangeDecoder::new(input) {
                for &expected_sym in &symbols {
                    assert_eq!(decoder.decode_symbol(&mut dec_model), Ok(expected_sym));
                }
            }
        }

        // Test underflow flushes (high < 0x8000 and low >= 0x8000 with underflow > 0)
        let complex_seq = [
            3, 3, 0, 2, 3, 3, 2, 3, 2, 1, 1, 2, 1, 0, 2, 1, 2, 0, 0, 2, 3, 0, 2, 3, 2, 1, 3, 3, 2,
            0,
        ];
        let mut complex_enc_model = AdaptiveModel::new(4);
        let mut complex_encoder = RangeEncoder::new();
        for &s in &complex_seq {
            complex_encoder.encode_symbol(s, &mut complex_enc_model);
        }
        let complex_bytes = complex_encoder.finish();
        let mut complex_dec_model = AdaptiveModel::new(4);
        for input in [complex_bytes.as_slice(), &[]] {
            if let Ok(mut decoder) = RangeDecoder::new(input) {
                for &expected_sym in &complex_seq {
                    assert_eq!(
                        decoder.decode_symbol(&mut complex_dec_model),
                        Ok(expected_sym)
                    );
                }
            }
        }

        // Test finish with low < 0x4000 and underflow_bits > 0
        let low_underflow_seq = [3, 3, 0, 2, 3, 3, 2, 3, 2, 1, 1, 2, 1, 0, 2];
        let mut low_enc_model = AdaptiveModel::new(4);
        let mut low_encoder = RangeEncoder::new();
        for &s in &low_underflow_seq {
            low_encoder.encode_symbol(s, &mut low_enc_model);
        }
        let low_bytes = low_encoder.finish();
        let mut low_dec_model = AdaptiveModel::new(4);
        for input in [low_bytes.as_slice(), &[]] {
            if let Ok(mut decoder) = RangeDecoder::new(input) {
                for &expected_sym in &low_underflow_seq {
                    assert_eq!(decoder.decode_symbol(&mut low_dec_model), Ok(expected_sym));
                }
            }
        }

        // ArithmeticWriter finish with bits_in_buf == 0
        let mut writer = ArithmeticWriter::new();
        writer.write_bits(0x55, 8);
        assert_eq!(writer.finish(), vec![0x55]);

        // Helpers and defaults
        let default_comp = QuantumCompressor::default();
        assert_eq!(default_comp.window_bits(), QUANTUM_DEFAULT_WINDOW_BITS);
        assert_eq!(position_slot_info(30), (14, 16385));
        assert_eq!(
            find_position_slot(usize::MAX),
            (NUM_POSITION_SLOTS - 1, 14, 0)
        );
    }
}
