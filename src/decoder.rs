use flate2::{Decompress, FlushDecompress, Status};

pub(crate) struct Decoder {
    pub decompressor: Decompress,
}

impl Decoder {
    // By default, a decompression process only considers the max_window_bits
    // retaining the same context for all the messages
    pub fn new() -> Self {
        let decompressor = Decompress::new(false);

        Self { decompressor }
    }

    pub fn decompress(&mut self, payload: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut decompressed_data = Vec::with_capacity(payload.len() * 2);
        let mut buffer = vec![0u8; 131072]; // Buffer to read decompressed chunks into
        let mut offset = 0;

        // Reset the decompressor before decompression to ensure no leftover state
        self.decompressor.reset(false);

        while offset < payload.len() {
            // Slice the remaining payload to decompress in chunks
            let input = &payload[offset..];

            // Decompress data in chunks into the buffer
            let status = self.decompressor.decompress(input, &mut buffer, FlushDecompress::Sync)?;

            // Only extend with actual data, trimming the trailing zeros
            let bytes_written = self.decompressor.total_out() as usize - decompressed_data.len();
            decompressed_data.extend_from_slice(&buffer[..bytes_written]);

            // Calculate how much was read and adjust offset
            let bytes_consumed = self.decompressor.total_in() as usize - offset;
            offset += bytes_consumed;

            // Break if the decompression is done
            if let Status::StreamEnd = status {
                break;
            }
        }

        Ok(decompressed_data)
    }
}