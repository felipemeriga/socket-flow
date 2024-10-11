use crate::error::Error;
use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;

// TODO - Will be used for future compression feature

// Helper function to compress the payload
#[allow(dead_code)]
pub fn compress_payload(payload: &[u8]) -> Result<Vec<u8>, Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload)?;
    let compressed_payload = encoder.finish()?;
    Ok(compressed_payload)
}

#[allow(dead_code)]
pub fn decompress_payload(payload: &[u8]) -> Result<Vec<u8>, Error> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(payload);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;

    Ok(decompressed_data)
}
