use bytes::BytesMut;
use flate2::{Decompress, FlushDecompress, Status};
use std::cmp;

fn calculate_buffer_size(payload_size: usize) -> usize {
    if payload_size <= 4096 {
        4096 // 4 KB for small payloads
    } else if payload_size <= 65536 {
        16384 // 16 KB for medium payloads
    } else {
        65536 // 64 KB for large payloads
    }
}

pub(crate) struct Decoder {
    pub decompressor: Decompress,
}

impl Decoder {
    pub fn new() -> Self {
        let decompressor = Decompress::new(false);
        Self { decompressor }
    }

    pub fn decompress(&mut self, payload: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        // adjust the buffer size, depending on the payload,
        // for balancing between CPU vs. Memory usage
        let buffer_size = calculate_buffer_size(payload.len());
        // Create an output buffer with a reasonable initial capacity
        let mut decompressed_data = BytesMut::with_capacity(payload.len() * 2);

        // Create a reusable buffer for intermediate decompression chunks
        let mut buffer = BytesMut::with_capacity(buffer_size);
        buffer.resize(buffer_size, 0);

        let mut offset = 0;

        // Reset the decompressor before starting to ensure no leftover state
        self.decompressor.reset(false);

        while offset < payload.len() {
            let input = &payload[offset..];

            // Decompress data into the reusable buffer
            let status = self
                .decompressor
                .decompress(input, &mut buffer, FlushDecompress::Sync)?;

            // Append decompressed bytes directly to `decompressed_data`
            let bytes_written = self.decompressor.total_out() as usize - decompressed_data.len();
            decompressed_data.extend_from_slice(&buffer[..bytes_written]);

            // Update the offset based on the amount of input consumed
            let bytes_consumed = self.decompressor.total_in() as usize - offset;
            offset += bytes_consumed;

            // Stop if the decompression is complete
            if let Status::StreamEnd = status {
                break;
            }

            // Dynamically grow the buffer if necessary (adaptive sizing)
            if buffer.len() < buffer_size {
                let new_size = cmp::min(buffer.len() * 2, 65536); // Cap growth at 64KB
                buffer.resize(new_size, 0);
            }
        }

        // Convert `BytesMut` to `Vec<u8>` and return
        Ok(decompressed_data.to_vec())
    }
}
