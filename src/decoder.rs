use bytes::BytesMut;
use flate2::{Decompress, FlushDecompress, Status};
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
    pub reset_context: bool,
}

const DEFLATE_TRAILER: [u8; 4] = [0, 0, 255, 255];

impl Decoder {
    pub fn new(reset_context: bool, window_bits: Option<u8>) -> Self {
        let decompressor = if let Some(window_bits) = window_bits {
            Decompress::new_with_window_bits(false, window_bits)
        } else {
            Decompress::new(false)
        };
        Self { decompressor, reset_context }
    }

    pub fn decompress(&mut self, payload: &mut BytesMut) -> Result<Vec<u8>, std::io::Error> {
        payload.extend_from_slice(&DEFLATE_TRAILER);
        // adjust the buffer size, depending on the payload,
        // for balancing between CPU vs. Memory usage
        let buffer_size = calculate_buffer_size(payload.len());
        // Create an output buffer with a reasonable initial capacity
        let mut decompressed_data = BytesMut::with_capacity(buffer_size);

        // Create a reusable buffer for intermediate decompression chunks
        let mut buffer = Vec::with_capacity(buffer_size);

        // Reset the decompressor before starting to ensure no leftover state
        if self.reset_context {
            self.decompressor.reset(false);
        }

        let before_in = self.decompressor.total_in();

        // Here on the while loop, we need to use decompressor.total_in() method, because
        // when we don't need to reset the context between decompression processes,
        // the decompressor will keep the number of bytes decompressed, also the client
        // responsible for compressing the payload, which is also keeping the context, will send
        // smaller payloads, hopping that the receiver also is keeping the context
        // That is why the handshake part is really important, to ensure we don't have a
        // misalignment.
        while self.decompressor.total_in() - before_in < payload.as_ref().len() as u64 {
            let i = (self.decompressor.total_in() - before_in) as usize;
            let input = &payload[i..];

            // TODO - We are using decompress_vec, perhaps only decompress method should be
            // more performant, the only issue with that,
            // is that you need to manage the buffer manually
            match self.decompressor.decompress_vec(input, &mut buffer, FlushDecompress::Sync)? {
                Status::Ok => {
                    decompressed_data.extend_from_slice(buffer.as_ref());
                    buffer.clear();
                }
                Status::StreamEnd => break,
                _ => {}
            }
        }
        decompressed_data.truncate(decompressed_data.len());

        Ok(decompressed_data.to_vec())
    }
}