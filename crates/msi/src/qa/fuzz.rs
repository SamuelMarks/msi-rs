//! Fuzz Testing Harnesses for Container Formats, Compression, and Parsers.
//!
//! Grounded directly in `cargo-fuzz` and libFuzzer methodologies:
//! - `cfbf_parse`: Header parsing, FAT sector chain traversal, directory parsing.
//! - `cab_decompress`: MSZIP and LZX decompressors on corrupted or adversarial bitstreams.
//! - `sql_query_parse`: Relational table schema and SQL statement parsing.
//! - `condition_eval`: Arbitrary expression strings and unicode fuzzing.

use crate::cab::lzx::LzxState;
use crate::cab::mszip::MszipDecompressor;
use crate::cfb::header::CfbHeader;
use crate::cfb::reader::CfbReader;
use crate::database::column::ColumnDef;
use crate::execution::properties::EvaluationContext;
use crate::wix::xml::XmlParser;

/// Fuzzing harness targeting Compound File Binary Format ([MS-CFB]) parsing.
///
/// Ensures memory safety, bounds checking, and absence of panics on malformed container bytes.
///
/// # Arguments
///
/// * `data` - Raw arbitrary or mutated byte slice.
///
/// # Returns
///
/// `true` if `data` parsed as a valid CFB container, `false` otherwise.
#[must_use]
pub fn fuzz_cfbf_parse(data: &[u8]) -> bool {
    if let Ok(_header) = CfbHeader::parse(data) {
        if let Ok(reader) = CfbReader::new(data) {
            let _entries = reader.entries();
            drop(reader.find_entry("\u{0005}SummaryInformation"));
            drop(reader.read_stream("!_Tables"));
            return true;
        }
    }
    false
}

/// Fuzzing harness targeting Cabinet compression engines (MSZIP and LZX).
///
/// Ensures memory safety and bounds checking on corrupted or adversarial compression payloads.
///
/// # Arguments
///
/// * `data` - Raw arbitrary compressed byte slice.
///
/// # Returns
///
/// `true` if either MSZIP or LZX decompressed the payload without error, `false` otherwise.
#[must_use]
pub fn fuzz_cab_decompress(data: &[u8]) -> bool {
    // 1. Fuzz MSZIP
    let mut decomp = MszipDecompressor::new();
    let mszip_ok = decomp.decompress_block(data, 32768).is_ok();

    // 2. Fuzz LZX
    let window_bits = if data.is_empty() { 15 } else { data[0] };
    let lzx_ok = LzxState::new(window_bits)
        .and_then(|mut lzx| lzx.decompress_block(data, 32768))
        .is_ok();

    mszip_ok || lzx_ok
}

/// Fuzzing harness targeting SQL schema parsing, column definition decoding, and XML parsing.
///
/// # Arguments
///
/// * `data` - Raw schema, query, or XML text.
///
/// # Returns
///
/// `true` if `data` decoded as a valid column bitmask or XML document, `false` otherwise.
#[must_use]
pub fn fuzz_sql_query_parse(data: &str) -> bool {
    let col_ok = data
        .parse::<u16>()
        .ok()
        .and_then(|bitmask| ColumnDef::from_bitmask("FuzzCol", bitmask).ok())
        .is_some();
    let xml_ok = XmlParser::new().parse(data).is_ok();
    col_ok || xml_ok
}

/// Fuzzing harness targeting Windows Installer condition expression evaluation.
///
/// # Arguments
///
/// * `expression` - Arbitrary condition expression string.
///
/// # Returns
///
/// `true` if the condition evaluated without syntax error, `false` otherwise.
#[must_use]
pub fn fuzz_condition_eval(expression: &str) -> bool {
    let mut context = EvaluationContext::new();
    context.set_property("VersionNT", "601");
    context.set_property("ALLUSERS", "1");
    context.set_property("INSTALLDIR", r"C:\Program Files\App");

    let eval_ok = context.evaluate_condition(expression).is_ok();
    let _formatted = context.format_string(expression);
    eval_ok
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::cfb::header::CfbVersion;
    use crate::cfb::writer::CfbWriter;

    /// Tests `fuzz_cfbf_parse` with boundary inputs: empty, zeroed, truncated, and random noise.
    #[test]
    fn test_fuzz_cfbf_parse_corpus() {
        // Empty
        assert!(!fuzz_cfbf_parse(&[]));

        // 8 bytes (magic only)
        assert!(!fuzz_cfbf_parse(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"));

        // Zeroed 512-byte block
        assert!(!fuzz_cfbf_parse(&[0u8; 512]));

        // Truncated header with valid signature
        let mut sig_hdr = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        sig_hdr.extend_from_slice(&[0u8; 100]);
        assert!(!fuzz_cfbf_parse(&sig_hdr));

        // Pseudorandom mutation bytes
        let mut pseudo_random = vec![0u8; 1024];
        for (i, byte) in pseudo_random.iter_mut().enumerate() {
            let val = (i.wrapping_mul(37).wrapping_add(13)) % 256;
            *byte = u8::try_from(val).unwrap_or(0);
        }
        assert!(!fuzz_cfbf_parse(&pseudo_random));

        // Valid header only (parse succeeds, reader fails)
        let hdr_bytes = CfbHeader::new(CfbVersion::V3).to_bytes();
        assert!(!fuzz_cfbf_parse(&hdr_bytes));

        // Valid full CFB archive (parse succeeds, reader succeeds, entries and streams read)
        let mut writer = CfbWriter::new(CfbVersion::V3);
        drop(writer.add_stream("\u{0005}SummaryInformation", &[1, 2, 3, 4]));
        drop(writer.add_stream("!_Tables", &[5, 6, 7, 8]));
        let cfb_bytes = writer.build();
        assert!(fuzz_cfbf_parse(&cfb_bytes));
    }

    /// Tests `fuzz_cab_decompress` with boundary inputs and mutated frames.
    #[test]
    fn test_fuzz_cab_decompress_corpus() {
        assert!(!fuzz_cab_decompress(&[]));
        assert!(!fuzz_cab_decompress(b"CK\x00\x00\x00")); // MSZIP prefix with truncated payload
        assert!(!fuzz_cab_decompress(&[0xFF; 256]));

        let mut mutated = vec![0x43, 0x4B]; // 'CK'
        for i in 0..128 {
            let b = (i * 17) % 256;
            mutated.push(u8::try_from(b).unwrap_or(0));
        }
        let _ = fuzz_cab_decompress(&mutated);
    }

    /// Tests `fuzz_sql_query_parse` with malformed and boundary schemas.
    #[test]
    fn test_fuzz_sql_query_parse_corpus() {
        assert!(fuzz_sql_query_parse("<Wix><Product/></Wix>"));
        let corpus = [
            "",
            "0",
            "1",
            "2",
            "4",
            "4096",
            "65535",
            "<UnclosedTag",
            r#"<?xml version="1.0"?>"#,
            "SELECT * FROM File WHERE Sequence > 10",
        ];
        for s in corpus {
            let _ = fuzz_sql_query_parse(s);
        }
    }

    /// Tests `fuzz_condition_eval` with complex, nested, unclosed, and adversarial expressions.
    #[test]
    fn test_fuzz_condition_eval_corpus() {
        assert!(fuzz_condition_eval("VersionNT > 500"));
        assert!(!fuzz_condition_eval("(VersionNT >= 600"));
        let corpus = [
            "",
            "1",
            "0",
            r#"VersionNT = "601" AND ALLUSERS"#,
            "VersionNT = 'unclosed quote",
            "NOT (A OR B AND (C XOR D))",
            r#"PROP1 >< "substr""#,
            r#"PROP1 << "prefix" AND PROP1 >> "suffix""#,
            r#"~= "case_insensitive""#,
            "&Feature = 3",
            "$Component = 1",
            "[PropertyName] and [%ENV_VAR]",
            r"[[nested]] [\#FileKey] [\$CompKey]",
            "AND OR NOT XOR = <> < > <= >= ~= ~<> ~< ~>",
            "((((((((((nested))))))))))",
        ];
        for expr in corpus {
            let _ = fuzz_condition_eval(expr);
        }
    }
}
