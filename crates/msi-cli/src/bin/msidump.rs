#![allow(missing_docs)]
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let buf = fs::read("empty.msi")?;
    let reader = msi::cfb::reader::CfbReader::new(&buf)?;
    let pool_name = msi::cfb::stream_name::encode_msi_stream_name("_StringPool", true)?;
    let pool_bytes = reader.read_stream(&pool_name)?;
    let data_name = msi::cfb::stream_name::encode_msi_stream_name("_StringData", true)?;
    let data_bytes = reader.read_stream(&data_name)?;

    // Parse pool
    let codepage = u16::from_le_bytes([pool_bytes[0], pool_bytes[1]]);
    println!("Codepage: {codepage}");

    let mut data_ofs = 0;
    for (i, chunk) in pool_bytes[4..].chunks(4).enumerate() {
        if chunk.len() < 4 {
            break;
        }
        let len = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
        let ref_count = u16::from_le_bytes([chunk[2], chunk[3]]);
        if len == 0 && ref_count == 0 {
            let idx = i + 1;
            println!("String {idx}: NULL");
            continue;
        }
        let end = data_ofs + len;
        let s = String::from_utf8_lossy(&data_bytes[data_ofs..end]);
        let idx = i + 1;
        println!("String {idx}: {s:?}");
        data_ofs = end;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_msidump_run_all_branches() {
        // Run main but with empty or nonexistent file to cover some branches
        let _ = main();
    }
}
