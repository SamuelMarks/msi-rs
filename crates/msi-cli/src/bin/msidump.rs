#![allow(clippy::similar_names)]

//! Internal utility for dumping MSI files.

use std::fs;

/// Dumps string pool of an MSI database.
///
/// # Arguments
///
/// * `path` - The path to the MSI file to dump.
///
/// # Errors
///
/// Returns a `Box<dyn std::error::Error>` on IO or parsing failure.
pub fn run_dump(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let buf = fs::read(path)?;
    let reader = msi::cfb::reader::CfbReader::new(&buf)?;
    let pool_name =
        msi::cfb::stream_name::encode_msi_stream_name("_StringPool", true).unwrap_or_default();
    let pool_bytes = reader.read_stream(&pool_name)?;
    let data_name =
        msi::cfb::stream_name::encode_msi_stream_name("_StringData", true).unwrap_or_default();
    let data_bytes = reader.read_stream(&data_name)?;

    // Parse pool
    let codepage = u16::from_le_bytes([pool_bytes[0], pool_bytes[1]]);
    println!("Codepage: {codepage}");

    let mut data_ofs = 0;
    for (i, chunk) in pool_bytes[4..].chunks(4).enumerate() {
        if chunk.len() < 4 {
            break; // Coverage: chunk mismatch
        }
        let len = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
        let ref_count = u16::from_le_bytes([chunk[2], chunk[3]]);
        if len == 0 && ref_count == 0 {
            let idx = i + 1;
            println!("String {idx}: NULL");
            continue;
        }
        let mut end = data_ofs + len;
        // avoid panic on out of bounds if string data stream is corrupt
        if end > data_bytes.len() {
            end = data_bytes.len();
        }
        let s = String::from_utf8_lossy(&data_bytes[data_ofs..end]);
        let idx = i + 1;
        println!("String {idx}: {s:?}");
        data_ofs = end;
    }

    Ok(())
}

/// Internal logic for running msidump with specific arguments.
///
/// # Errors
///
/// Returns an error if the MSI file cannot be read or parsed.
fn run_main(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = if args.len() > 1 {
        &args[1]
    } else {
        "empty.msi"
    };
    run_dump(path)
}

/// Main entry point for the msidump utility.
///
/// # Errors
///
/// Returns an error if the MSI file cannot be read or parsed.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    run_main(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    #[test]
    fn test_msidump_run_all_branches() {
        use msi::cfb::header::CfbVersion;
        use msi::cfb::stream_name::encode_msi_stream_name;
        use msi::cfb::writer::CfbWriter;

        // Run main but with empty or nonexistent file to cover some branches
        let _ = run_dump("nonexistent.msi");

        let path = std::env::temp_dir().join("test_msidump.msi");

        let mut writer = CfbWriter::new(CfbVersion::V3);
        let pool_name = encode_msi_stream_name("_StringPool", true).unwrap();
        let data_name = encode_msi_stream_name("_StringData", true).unwrap();

        let mut pool_bytes = vec![0; 4]; // codepage 0, refcount 0
        pool_bytes.extend_from_slice(&[0, 0, 0, 0]); // String 1: len 0, ref_count 0
        pool_bytes.extend_from_slice(&[4, 0, 1, 0]); // String 2: len 4, ref_count 1
        pool_bytes.extend_from_slice(&[0, 0, 1, 0]); // String 3: len 0, ref_count 1

        writer.add_stream(&pool_name, &pool_bytes).unwrap();
        writer.add_stream(&data_name, b"test").unwrap();

        let cfb_data = writer.build();
        let _ = fs::write(&path, cfb_data);

        let res = run_dump(path.to_str().unwrap_or(""));
        assert!(res.is_ok());

        let _ = fs::remove_file(&path);

        // test invalid cfb
        let _ = fs::write(&path, b"invalid cfb header data blah blah");

        let res_invalid = run_dump(path.to_str().unwrap_or(""));
        assert!(res_invalid.is_err());
        let _ = fs::remove_file(&path);

        // test missing pool stream
        let cfbg_no_pool = CfbWriter::new(CfbVersion::V4);
        let cfb_no_pool = cfbg_no_pool.build();
        let _ = fs::write(&path, cfb_no_pool);
        let res_no_pool = run_dump(path.to_str().unwrap_or(""));
        assert!(res_no_pool.is_err());
        let _ = fs::remove_file(&path);

        // test missing data stream
        let mut cfbg_no_data = CfbWriter::new(CfbVersion::V4);
        let pool_name = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let _ = cfbg_no_data.add_stream(&pool_name, &[0, 0, 0, 0]);
        let cfb_no_data = cfbg_no_data.build();
        let _ = fs::write(&path, cfb_no_data);
        let res_no_data = run_dump(path.to_str().unwrap_or(""));
        assert!(res_no_data.is_err());
        let _ = fs::remove_file(&path);

        // test break on short chunk
        let mut cfbg = CfbWriter::new(CfbVersion::V4);
        let pool_name = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let data_name = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        // 4 bytes header, 2 bytes partial chunk (should trigger break)
        let _ = cfbg.add_stream(&pool_name, &[0, 0, 0, 0, 1, 2]);
        let _ = cfbg.add_stream(&data_name, &[]);
        let bad_cfb = cfbg.build();
        let _ = fs::write(&path, bad_cfb);
        let res_short = run_dump(path.to_str().unwrap_or(""));
        assert!(res_short.is_ok()); // parse succeeds but breaks early
        let _ = fs::remove_file(&path);

        // test out of bounds end
        let mut cfbg2 = CfbWriter::new(CfbVersion::V4);
        // 4 bytes header, chunk says len 100 but data is 0 len
        let _ = cfbg2.add_stream(&pool_name, &[0, 0, 0, 0, 100, 0, 1, 0]);
        let _ = cfbg2.add_stream(&data_name, &[]);
        let bad_cfb2 = cfbg2.build();
        let _ = fs::write(&path, bad_cfb2);
        let res_oob = run_dump(path.to_str().unwrap_or(""));
        assert!(res_oob.is_ok()); // parse succeeds but breaks early
        let _ = fs::remove_file(&path);

        // test null string
        let mut cfbg3 = CfbWriter::new(CfbVersion::V4);
        let _ = cfbg3.add_stream(&pool_name, &[0, 0, 0, 0, 0, 0, 0, 0]); // 4 header, 4 null chunk
        let _ = cfbg3.add_stream(&data_name, &[]);
        let null_cfb = cfbg3.build();
        let _ = fs::write(&path, null_cfb);
        let res_null = run_dump(path.to_str().unwrap_or(""));
        assert!(res_null.is_ok());
        let _ = fs::remove_file(&path);

        // test valid string
        let mut cfbg_valid = CfbWriter::new(CfbVersion::V4);
        let _ = cfbg_valid.add_stream(&pool_name, &[0, 0, 0, 0, 4, 0, 1, 0]); // 4 header, len=4, ref=1
        let _ = cfbg_valid.add_stream(&data_name, b"test");
        let valid_cfb = cfbg_valid.build();
        let _ = fs::write(&path, valid_cfb);
        let res_valid = run_dump(path.to_str().unwrap_or(""));
        assert!(res_valid.is_ok());
        let _ = fs::remove_file(&path);

        let args_res = run_main(&["msidump".to_string(), "empty.msi".to_string()]);
        assert!(args_res.is_err()); // because empty.msi doesn't exist or is invalid

        let args_res_empty = run_main(&["msidump".to_string()]);
        assert!(args_res_empty.is_err());

        // Also just cover main() directly for the 100%
        let _ = main();
    }
}
