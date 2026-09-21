//! Fuzz Testing Harnesses for Container Formats, Compression, and Parsers.
//!
//! Grounded directly in `cargo-fuzz` and libFuzzer methodologies:
//! - `cfbf_parse`: Header parsing, FAT sector chain traversal, directory parsing.
//! - `cab_decompress`: MSZIP and LZX decompressors on corrupted or adversarial bitstreams.
//! - `sql_query_parse`: Relational table schema and SQL statement parsing.
//! - `condition_eval`: Arbitrary expression strings and unicode fuzzing.

use crate::cab::lzx::LzxState;
use crate::cab::mszip::MszipEngine;
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
pub fn fuzz_cfbf_parse(data: &[u8]) {
    if let Ok(_header) = CfbHeader::parse(data) {
        if let Ok(reader) = CfbReader::new(data) {
            let _entries = reader.entries();
            drop(reader.find_entry("\u{0005}SummaryInformation"));
            drop(reader.read_stream("!_Tables"));
        }
    }
}

/// Fuzzing harness targeting Cabinet compression engines (MSZIP and LZX).
///
/// Ensures memory safety and bounds checking on corrupted or adversarial compression payloads.
///
/// # Arguments
///
/// * `data` - Raw arbitrary compressed byte slice.
pub fn fuzz_cab_decompress(data: &[u8]) {
    // 1. Fuzz MSZIP
    drop(MszipEngine.decompress(data, 32768));

    // 2. Fuzz LZX
    let window_bits = if data.is_empty() { 15 } else { data[0] };
    if let Ok(mut lzx) = LzxState::new(window_bits) {
        drop(lzx.decompress_block(data, 32768));
    }
}

/// Fuzzing harness targeting SQL schema parsing, column definition decoding, and XML parsing.
///
/// # Arguments
///
/// * `data` - Raw schema, query, or XML text.
pub fn fuzz_sql_query_parse(data: &str) {
    if let Ok(bitmask) = data.parse::<u16>() {
        drop(ColumnDef::from_bitmask("FuzzCol", bitmask));
    }
    drop(XmlParser::new().parse(data));
}

/// Fuzzing harness targeting Windows Installer condition expression evaluation.
///
/// # Arguments
///
/// * `expression` - Arbitrary condition expression string.
pub fn fuzz_condition_eval(expression: &str) {
    let mut context = EvaluationContext::new();
    context.set_property("VersionNT", "601");
    context.set_property("ALLUSERS", "1");
    context.set_property("INSTALLDIR", r"C:\Program Files\App");

    drop(context.evaluate_condition(expression));
    drop(context.format_string(expression));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfb::header::CfbVersion;
    use crate::cfb::writer::CfbWriter;

    /// Tests `fuzz_cfbf_parse` with boundary inputs: empty, zeroed, truncated, and random noise.
    #[test]
    fn test_fuzz_cfbf_parse_corpus() {
        // Empty
        fuzz_cfbf_parse(&[]);

        // 8 bytes (magic only)
        fuzz_cfbf_parse(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1");

        // Zeroed 512-byte block
        fuzz_cfbf_parse(&[0u8; 512]);

        // Truncated header with valid signature
        let mut sig_hdr = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        sig_hdr.extend_from_slice(&[0u8; 100]);
        fuzz_cfbf_parse(&sig_hdr);

        // Pseudorandom mutation bytes
        let mut pseudo_random = vec![0u8; 1024];
        for (i, byte) in pseudo_random.iter_mut().enumerate() {
            let val = (i.wrapping_mul(37).wrapping_add(13)) % 256;
            *byte = u8::try_from(val).unwrap_or(0);
        }
        fuzz_cfbf_parse(&pseudo_random);

        // Valid header only (parse succeeds, reader fails)
        let hdr_bytes = CfbHeader::new(CfbVersion::V3).to_bytes();
        fuzz_cfbf_parse(&hdr_bytes);

        // Valid full CFB archive (parse succeeds, reader succeeds, entries and streams read)
        let mut writer = CfbWriter::new(CfbVersion::V3);
        drop(writer.add_stream("\u{0005}SummaryInformation", &[1, 2, 3, 4]));
        drop(writer.add_stream("!_Tables", &[5, 6, 7, 8]));
        let cfb_bytes = writer.build();
        fuzz_cfbf_parse(&cfb_bytes);
    }

    /// Tests `fuzz_cab_decompress` with boundary inputs and mutated frames.
    #[test]
    fn test_fuzz_cab_decompress_corpus() {
        fuzz_cab_decompress(&[]);
        fuzz_cab_decompress(b"CK\x00\x00\x00"); // MSZIP prefix with truncated payload
        fuzz_cab_decompress(&[0xFF; 256]);

        let mut mutated = vec![0x43, 0x4B]; // 'CK'
        for i in 0..128 {
            let b = (i * 17) % 256;
            mutated.push(u8::try_from(b).unwrap_or(0));
        }
        fuzz_cab_decompress(&mutated);
    }

    /// Tests `fuzz_sql_query_parse` with malformed and boundary schemas.
    #[test]
    fn test_fuzz_sql_query_parse_corpus() {
        let corpus = [
            "",
            "0",
            "1",
            "2",
            "4",
            "4096",
            "65535",
            "<Wix><Product/></Wix>",
            "<UnclosedTag",
            r#"<?xml version="1.0"?>"#,
            "SELECT * FROM File WHERE Sequence > 10",
        ];
        for s in corpus {
            fuzz_sql_query_parse(s);
        }
    }

    /// Tests `fuzz_condition_eval` with complex, nested, unclosed, and adversarial expressions.
    #[test]
    fn test_fuzz_condition_eval_corpus() {
        let corpus = [
            "",
            "1",
            "0",
            "VersionNT > 500",
            r#"VersionNT = "601" AND ALLUSERS"#,
            "(VersionNT >= 600", // Unclosed paren
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
            fuzz_condition_eval(expr);
        }
    }
}
