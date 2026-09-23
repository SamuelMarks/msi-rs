//! MSI stream name encoding and decoding schemes ([MS-CFB] & MSI SDK).

use crate::error::{Error, Result};

/// Stream name prefix for OLE Summary Information stream (`\u{0005}`).
pub const SUMMARY_INFORMATION_PREFIX: char = '\u{0005}';

/// Exact CFB stream name for the standard Summary Information stream.
pub const SUMMARY_INFORMATION_STREAM: &str = "\u{0005}SummaryInformation";

/// Exact CFB stream name for the standard Digital Signature stream.
pub const DIGITAL_SIGNATURE_STREAM: &str = "\u{0005}DigitalSignature";

/// Base offset for compressed MSI two-character pairs (`0x3800`).
pub const MSI_NAME_COMPRESSION_BASE: u16 = 0x3800;

/// Base offset for odd single trailing character (`0x4800`).
pub const MSI_NAME_SINGLE_CHAR_BASE: u16 = 0x4800;

/// Special MSI table stream name prefix code unit (`0x4840`).
pub const MSI_TABLE_STREAM_PREFIX: u16 = 0x4840;

/// The 64-character alphabet used for MSI stream name compression.
const ALPHABET: &[u8; 64] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_.";

/// Converts an ASCII character in the 64-character subset into its 6-bit index (`0..=63`).
///
/// # Arguments
///
/// * `ch` - Character to convert.
///
/// # Returns
///
/// 6-bit integer index.
///
/// # Errors
///
/// Returns [`Error::InvalidStreamName`] if character is not in the 64-character alphabet.
pub fn char_to_index(ch: char) -> Result<u16> {
    match ch {
        '0'..='9' => Ok(ch as u16 - '0' as u16),
        'a'..='z' => Ok(ch as u16 - 'a' as u16 + 10),
        'A'..='Z' => Ok(ch as u16 - 'A' as u16 + 36),
        '_' => Ok(62),
        '.' => Ok(63),
        _ => Err(Error::InvalidStreamName {
            name: ch.to_string(),
            reason: format!("character '{ch}' is not in the MSI 64-character subset"),
        }),
    }
}

/// Converts a 6-bit index (`0..=63`) back to its ASCII character.
///
/// # Arguments
///
/// * `index` - 6-bit integer index.
///
/// # Returns
///
/// ASCII character from the 64-character alphabet.
///
/// # Errors
///
/// Returns [`Error::InvalidStreamName`] if index is 64 or greater.
pub fn index_to_char(index: u16) -> Result<char> {
    if index < 64 {
        Ok(ALPHABET[index as usize] as char)
    } else {
        Err(Error::InvalidStreamName {
            name: index.to_string(),
            reason: format!("alphabet index {index} out of bounds (max 63)"),
        })
    }
}

/// Converts a sequence of UTF-16 code units into a validated [`String`].
///
/// # Arguments
///
/// * `units` - Slice of UTF-16 code units.
/// * `name` - Identifier for error reporting.
///
/// # Returns
///
/// A valid [`String`].
///
/// # Errors
///
/// Returns [`Error::InvalidStreamName`] if the units contain unpaired UTF-16 surrogates.
pub fn utf16_units_to_string(units: &[u16], name: &str) -> Result<String> {
    String::from_utf16(units).map_err(|err| Error::InvalidStreamName {
        name: name.to_string(),
        reason: format!("failed to form valid UTF-16 stream name: {err}"),
    })
}

/// Encodes an arbitrary table or stream name using the MSI 64-character packing scheme.
///
/// # Arguments
///
/// * `name` - The plain string name to compress.
/// * `is_table` - If `true`, prefixes with `0x4840` (`!`) to designate a primary table stream.
///
/// # Returns
///
/// The encoded stream name as a [`String`].
///
/// # Errors
///
/// Returns [`Error::InvalidStreamName`] if any character cannot be represented.
pub fn encode_msi_stream_name(name: &str, is_table: bool) -> Result<String> {
    if name.is_empty() {
        return Ok(String::new());
    }

    let chars: Vec<char> = name.chars().collect();
    let mut encoded_units = Vec::new();

    if is_table {
        encoded_units.push(MSI_TABLE_STREAM_PREFIX);
    }

    let mut i = 0;
    while i < chars.len() {
        let ch0 = chars[i];
        let val0 = char_to_index(ch0)?;
        i += 1;

        if i < chars.len() {
            let ch1 = chars[i];
            let val1 = char_to_index(ch1)?;
            i += 1;
            let code_unit = MSI_NAME_COMPRESSION_BASE + val0 + (val1 * 64);
            encoded_units.push(code_unit);
        } else {
            // Trailing single odd character: encoded with MSI_NAME_SINGLE_CHAR_BASE (0x4800..0x483F)
            let code_unit = MSI_NAME_SINGLE_CHAR_BASE + val0;
            encoded_units.push(code_unit);
        }
    }

    utf16_units_to_string(&encoded_units, name)
}

/// Decodes an MSI compressed stream name back into its plain string representation.
///
/// # Arguments
///
/// * `encoded` - The encoded stream name.
///
/// # Returns
///
/// A tuple containing `(decoded_name, is_table)`.
///
/// # Errors
///
/// Returns [`Error::InvalidStreamName`] if decoding encounters malformed words.
#[allow(clippy::cast_possible_truncation)]
pub fn decode_msi_stream_name(encoded: &str) -> Result<(String, bool)> {
    if encoded.is_empty() {
        return Ok((String::new(), false));
    }

    // Special case check for Summary Information and Digital Signature
    if encoded == SUMMARY_INFORMATION_STREAM || encoded == DIGITAL_SIGNATURE_STREAM {
        return Ok((encoded.to_string(), false));
    }

    let mut decoded = String::new();
    let mut is_table = false;

    for (idx, ch) in encoded.chars().enumerate() {
        let unit = ch as u32;
        if idx == 0 && unit == u32::from(MSI_TABLE_STREAM_PREFIX) {
            is_table = true;
            continue;
        }

        if (u32::from(MSI_NAME_COMPRESSION_BASE)..u32::from(MSI_NAME_SINGLE_CHAR_BASE))
            .contains(&unit)
        {
            let offset = unit.wrapping_sub(u32::from(MSI_NAME_COMPRESSION_BASE)) as usize;
            let val0 = offset % 64;
            let val1 = offset / 64;
            decoded.push(ALPHABET[val0] as char);
            decoded.push(ALPHABET[val1] as char);
        } else if (u32::from(MSI_NAME_SINGLE_CHAR_BASE)..u32::from(MSI_NAME_SINGLE_CHAR_BASE) + 64)
            .contains(&unit)
        {
            let val0 = unit.wrapping_sub(u32::from(MSI_NAME_SINGLE_CHAR_BASE)) as usize;
            decoded.push(ALPHABET[val0] as char);
        } else {
            decoded.push(ch);
        }
    }

    Ok((decoded, is_table))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests char to index conversion and back.
    #[test]
    fn test_char_index_roundtrip() {
        for i in 0u16..64u16 {
            let ch = ALPHABET[i as usize] as char;
            assert_eq!(char_to_index(ch), Ok(i));
            assert_eq!(index_to_char(i), Ok(ch));
        }

        assert!(char_to_index('?').is_err());
        assert!(index_to_char(64).is_err());
    }

    /// Tests converting UTF-16 code units to string including error handling.
    #[test]
    fn test_utf16_units_to_string() {
        let units = [0x0041, 0x0042];
        assert_eq!(utf16_units_to_string(&units, "AB"), Ok("AB".to_string()));

        // Lone surrogate
        let bad = [0xD800];
        assert!(utf16_units_to_string(&bad, "bad").is_err());
    }

    /// Tests MSI stream name compression roundtrip for regular streams and table streams.
    #[test]
    fn test_msi_stream_name_roundtrip() {
        // Even length regular stream
        let enc1 = encode_msi_stream_name("Component", false);
        let dec1 = enc1.and_then(|s| decode_msi_stream_name(&s));
        assert_eq!(dec1, Ok(("Component".to_string(), false)));

        // Odd length table stream
        let enc2 = encode_msi_stream_name("File", true);
        let dec2 = enc2.and_then(|s| decode_msi_stream_name(&s));
        assert_eq!(dec2, Ok(("File".to_string(), true)));

        // Single character table
        let enc3 = encode_msi_stream_name("A", true);
        let dec3 = enc3.and_then(|s| decode_msi_stream_name(&s));
        assert_eq!(dec3, Ok(("A".to_string(), true)));

        // Empty name
        assert_eq!(encode_msi_stream_name("", false), Ok(String::new()));
        assert_eq!(decode_msi_stream_name(""), Ok((String::new(), false)));

        // Error encoding unsupported character at both ch0 and ch1 positions
        assert!(encode_msi_stream_name("!bad", false).is_err());
        assert!(encode_msi_stream_name("bad!name", false).is_err());
        assert!(encode_msi_stream_name("first!bad", false).is_err());
    }

    /// Tests special MSI streams like Summary Information and Digital Signature.
    #[test]
    fn test_special_msi_streams() {
        assert_eq!(
            decode_msi_stream_name(SUMMARY_INFORMATION_STREAM),
            Ok((SUMMARY_INFORMATION_STREAM.to_string(), false))
        );
        assert_eq!(
            decode_msi_stream_name(DIGITAL_SIGNATURE_STREAM),
            Ok((DIGITAL_SIGNATURE_STREAM.to_string(), false))
        );
    }

    /// Tests uncompressed stream fallback in decoder.
    #[test]
    fn test_uncompressed_stream_name() {
        let uncompressed = "PlainName";
        let dec = decode_msi_stream_name(uncompressed);
        assert_eq!(dec, Ok(("PlainName".to_string(), false)));
    }
}
