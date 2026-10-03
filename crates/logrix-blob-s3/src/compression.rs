use std::io::{Cursor, Read, Write};

/// Default Zstandard compression level (fast indexing throughput with ~80% ratio).
pub const DEFAULT_ZSTD_LEVEL: i32 = 3;

/// Compress byte buffer using Zstandard.
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>, std::io::Error> {
    let compression_level = if level <= 0 {
        DEFAULT_ZSTD_LEVEL
    } else {
        level
    };
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), compression_level)?;
    encoder.write_all(data)?;
    encoder.finish()
}

/// Decompress byte buffer previously compressed with Zstandard.
pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = zstd::stream::Decoder::new(Cursor::new(data))?;
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let original = b"{\"block\": 19500000, \"hash\": \"0xabcdef1234567890\", \"logs\": []}";
        let compressed = compress_zstd(original, DEFAULT_ZSTD_LEVEL).expect("compression");
        let decompressed = decompress_zstd(&compressed).expect("decompression");
        assert_eq!(original.to_vec(), decompressed);
    }
}
