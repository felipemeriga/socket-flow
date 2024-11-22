use bytes::BytesMut;
use flate2::{Compress, Compression, FlushCompress, Status};

pub struct Encoder {
    pub compressor: Compress,
    pub reset_context: bool,
}

impl Encoder {
    pub fn new(reset_context: bool, window_bits: Option<u8>) -> Self {
        let compressor = if let Some(window_bits) = window_bits {
            Compress::new_with_window_bits(Compression::default(), false, window_bits)
        } else {
            Compress::new(Compression::default(), false)
        };

        Self { compressor, reset_context }
    }

    pub fn compress(&mut self, payload: &mut BytesMut) -> Result<Vec<u8>, std::io::Error> {
        if payload.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare a buffer for intermediate compressed data
        let mut compressed_data = Vec::with_capacity(payload.len() * 2);

        let before_in = self.compressor.total_in();

        // Incremental compression loop
        while self.compressor.total_in() - before_in < payload.as_ref().len() as u64 {
            let i = (self.compressor.total_in() - before_in) as usize;
            let input = &payload[i..];

            match self
                .compressor
                .compress_vec(input, &mut compressed_data, FlushCompress::Sync)?
            {
                Status::Ok => continue,
                Status::StreamEnd => break,
                Status::BufError => {
                    // Dynamically grow the buffer if needed
                    compressed_data.reserve((compressed_data.len() as f64 * 1.5) as usize);
                }
            }
        }

        // Ensure the trailer is present
        while !compressed_data.ends_with(&[0, 0, 0xFF, 0xFF]) {
            compressed_data.reserve(5);
            match self
                .compressor
                .compress_vec(&[], &mut compressed_data, FlushCompress::Sync)?
            {
                Status::Ok | Status::BufError => continue,
                Status::StreamEnd => break,
            }
        }

        // Remove the DEFLATE trailer
        compressed_data.truncate(compressed_data.len() - 4);

        // Reset the compressor if needed
        if self.reset_context {
            self.compressor.reset();
        }

        // Return the compressed data
        Ok(compressed_data)
    }
}
