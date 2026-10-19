//! Canonical Huffman prefix tree encoding and decoding for Microsoft LZX.

use super::decoder::LzxBitReader;
use crate::error::{MsiError, Result};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Canonical prefix Huffman decoding and encoding tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuffmanTree {
    /// Binary tree nodes stored as `[left_child, right_child]`.
    ///
    /// If high bit `0x8000` is set, the node represents a leaf symbol (`val & 0x7FFF`).
    pub(crate) nodes: Vec<[u16; 2]>,
    /// Array of symbol code lengths (0 means symbol is unused).
    pub(crate) code_lengths: Vec<u8>,
}

impl HuffmanTree {
    /// Builds a canonical [`HuffmanTree`] from symbol code lengths.
    ///
    /// # Arguments
    ///
    /// * `lengths` - Slice of code lengths for each symbol in the alphabet.
    ///
    /// # Returns
    ///
    /// A constructed [`HuffmanTree`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::LzxDecompressionFailed`] if the code lengths represent an oversubscribed or invalid tree.
    pub fn from_lengths(lengths: &[u8]) -> Result<Self> {
        let mut count = [0u32; 17];
        for &len in lengths {
            if len > 16 {
                return Err(MsiError::LzxDecompressionFailed {
                    reason: format!("Huffman code length {len} exceeds 16 bits"),
                });
            }
            if len > 0 {
                count[usize::from(len)] += 1;
            }
        }

        // Kraft-McMillan inequality verification: sum(count[l] * 2^(16-l)) <= 2^16
        let mut kraft_sum = 0u32;
        for (l, &cnt) in count.iter().enumerate().skip(1) {
            kraft_sum = kraft_sum.saturating_add(cnt.saturating_mul(1u32 << (16 - l)));
        }
        if kraft_sum > (1u32 << 16) {
            return Err(MsiError::LzxDecompressionFailed {
                reason: "oversubscribed Huffman tree".to_string(),
            });
        }

        let mut next_code = [0u32; 17];
        let mut code = 0u32;
        for l in 1..=16 {
            code = (code + count[l - 1]) << 1;
            next_code[l] = code;
        }

        let mut nodes: Vec<[u16; 2]> = Vec::new();
        nodes.push([0, 0]); // Root node 0

        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let sym_code = next_code[usize::from(len)];
            next_code[usize::from(len)] += 1;

            let mut curr = 0usize;
            for bit_idx in (0..len).rev() {
                let bit = usize::try_from((sym_code >> bit_idx) & 1).unwrap_or(0);
                if bit_idx > 0 {
                    if nodes[curr][bit] == 0 {
                        let new_node = u16::try_from(nodes.len()).unwrap_or(0);
                        nodes.push([0, 0]);
                        nodes[curr][bit] = new_node;
                    }
                    curr = usize::from(nodes[curr][bit]);
                } else {
                    let sym_u16 = u16::try_from(sym).unwrap_or(0);
                    nodes[curr][bit] = 0x8000 | sym_u16;
                }
            }
        }

        Ok(Self {
            nodes,
            code_lengths: lengths.to_vec(),
        })
    }

    /// Decodes a single symbol from the bitstream using the Huffman tree.
    ///
    /// # Arguments
    ///
    /// * `reader` - Bitstream reader.
    ///
    /// # Returns
    ///
    /// The decoded symbol integer.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::LzxDecompressionFailed`] on invalid bit sequences or corrupted tree states.
    pub fn decode_symbol(&self, reader: &mut LzxBitReader<'_>) -> Result<u16> {
        if self.nodes.is_empty() {
            return Err(MsiError::LzxDecompressionFailed {
                reason: "empty Huffman tree".to_string(),
            });
        }

        let mut curr = 0usize;
        loop {
            let bit = usize::from(reader.read_bit()?);
            let next = self.nodes[curr][bit];
            if next == 0 {
                return Err(MsiError::LzxDecompressionFailed {
                    reason: "invalid Huffman code in bitstream".to_string(),
                });
            }
            if (next & 0x8000) != 0 {
                return Ok(next & 0x7FFF);
            }
            curr = usize::from(next);
            if curr >= self.nodes.len() {
                return Err(MsiError::LzxDecompressionFailed {
                    reason: "corrupted Huffman tree index".to_string(),
                });
            }
        }
    }

    /// Returns the symbol code lengths.
    ///
    /// # Returns
    ///
    /// Slice of code bit-lengths indexed by symbol number.
    #[must_use]
    pub fn code_lengths(&self) -> &[u8] {
        &self.code_lengths
    }

    /// Computes canonical `(code, length)` tuples for each symbol.
    ///
    /// # Returns
    ///
    /// Vector of `(canonical_code, bit_length)` pairs for every symbol in the alphabet.
    #[must_use]
    pub fn canonical_codes(&self) -> Vec<(u16, u8)> {
        let mut count = [0u32; 17];
        for &len in &self.code_lengths {
            if len > 0 {
                count[usize::from(len)] += 1;
            }
        }

        let mut next_code = [0u32; 17];
        let mut code = 0u32;
        for l in 1..=16 {
            code = (code + count[l - 1]) << 1;
            next_code[l] = code;
        }

        let mut codes = Vec::with_capacity(self.code_lengths.len());
        for &len in &self.code_lengths {
            if len == 0 {
                codes.push((0, 0));
            } else {
                let c = u16::try_from(next_code[usize::from(len)]).unwrap_or(0);
                next_code[usize::from(len)] += 1;
                codes.push((c, len));
            }
        }
        codes
    }
}

/// Decodes an array of code lengths using the pre-tree and delta encoding relative to previous lengths.
///
/// Implements Section 2.4.1 of the Microsoft LZX Specification.
///
/// # Arguments
///
/// * `reader` - Bitstream reader.
/// * `pre_tree` - Constructed pre-tree.
/// * `lengths` - Target slice to populate with decoded lengths.
/// * `prev_lengths` - Previous block's code lengths for delta calculation.
///
/// # Errors
///
/// Returns [`MsiError::LzxDecompressionFailed`] on invalid bitstream symbols.
///
/// # Returns
///
/// `Ok(())` when the Huffman code lengths are decoded.
pub fn decode_tree_lengths(
    reader: &mut LzxBitReader<'_>,
    pre_tree: &HuffmanTree,
    lengths: &mut [u8],
    prev_lengths: &[u8],
) -> Result<()> {
    let mut i = 0;
    while i < lengths.len() {
        let sym = pre_tree.decode_symbol(reader)?;
        if sym <= 16 {
            let prev = if i < prev_lengths.len() {
                prev_lengths[i]
            } else {
                0
            };
            let sym_u8 = u8::try_from(sym).unwrap_or(0);
            let val = (prev + 17 - sym_u8) % 17;
            lengths[i] = val;
            i += 1;
        } else if sym == 17 {
            let count = usize::from(reader.read_bits(4)?) + 4;
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = 0;
            }
            i = end;
        } else if sym == 18 {
            let count = usize::from(reader.read_bits(5)?) + 20;
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = 0;
            }
            i = end;
        } else {
            let count = usize::from(reader.read_bits(1)?) + 4;
            let next_sym = pre_tree.decode_symbol(reader)?;
            let prev = if i < prev_lengths.len() {
                prev_lengths[i]
            } else {
                0
            };
            let val = if next_sym <= 16 {
                let next_u8 = u8::try_from(next_sym).unwrap_or(0);
                (prev + 17 - next_u8) % 17
            } else {
                0
            };
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = val;
            }
            i = end;
        }
    }
    Ok(())
}

/// Generates optimal canonical Huffman code lengths from an alphabet frequency distribution.
///
/// # Arguments
///
/// * `freqs` - Frequency counts for each symbol.
/// * `max_bits` - Maximum allowed code length (e.g. 16).
///
/// # Returns
///
/// Vector of code lengths for each symbol.
#[must_use]
pub fn compute_huffman_lengths(freqs: &[u32], max_bits: u8) -> Vec<u8> {
    let active_symbols: Vec<(usize, u32)> = freqs
        .iter()
        .enumerate()
        .filter(|(_, &f)| f > 0)
        .map(|(i, &f)| (i, f))
        .collect();

    let mut lengths = vec![0u8; freqs.len()];
    if active_symbols.is_empty() {
        return lengths;
    }
    if active_symbols.len() == 1 {
        lengths[active_symbols[0].0] = 1;
        return lengths;
    }

    let mut current_weights: Vec<u64> = active_symbols.iter().map(|&(_, f)| u64::from(f)).collect();

    loop {
        let mut heap = BinaryHeap::new();
        let mut tree_nodes: Vec<(Option<usize>, Option<usize>)> = Vec::new();

        for (idx, &w) in current_weights.iter().enumerate() {
            let node_id = tree_nodes.len();
            tree_nodes.push((None, None));
            heap.push(Reverse((w, node_id, Some(idx))));
        }

        while let (Some(Reverse((w1, n1, _))), Some(Reverse((w2, n2, _)))) =
            (heap.pop(), heap.pop())
        {
            let parent_id = tree_nodes.len();
            tree_nodes.push((Some(n1), Some(n2)));
            heap.push(Reverse((w1.saturating_add(w2), parent_id, None)));
        }

        let root = tree_nodes.len().saturating_sub(1);
        let mut depths = vec![0u8; tree_nodes.len()];
        let mut stack = vec![(root, 0u8)];

        while let Some((node_idx, d)) = stack.pop() {
            depths[node_idx] = d;
            let (left, right) = tree_nodes[node_idx];
            if let Some(l) = left {
                stack.push((l, d.saturating_add(1)));
            }
            if let Some(r) = right {
                stack.push((r, d.saturating_add(1)));
            }
        }

        let max_depth = active_symbols
            .iter()
            .enumerate()
            .map(|(idx, _)| depths[idx])
            .max()
            .unwrap_or(0);

        if max_depth <= max_bits {
            for (idx, &(sym, _)) in active_symbols.iter().enumerate() {
                lengths[sym] = depths[idx].max(1);
            }
            break;
        }

        for w in &mut current_weights {
            *w = (*w).div_ceil(2);
        }
    }

    lengths
}
