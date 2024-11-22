// use bytes::BytesMut;
// use flate2::{Compress, Compression, FlushCompress, Status};
// use std::cmp;
//
// fn calculate_buffer_size(payload_size: usize) -> usize {
//     if payload_size <= 4096 {
//         4096 // 4 KB for small payloads
//     } else if payload_size <= 65536 {
//         16384 // 16 KB for medium payloads
//     } else {
//         65536 // 64 KB for large payloads
//     }
// }
// pub(crate) struct Encoder {
//     compressor: Compress,
// }
//
//
// impl Encoder {
//     /// Creates a new encoder with a default compression level and window size (15 bits).
//     pub fn new() -> Self {
//         let compressor = Compress::new_with_window_bits(Compression::default(), false, 15);
//         Self { compressor }
//     }
//
//     /// Compresses the input payload and returns the compressed data as a `Vec<u8>`.
//     pub fn compress(&mut self, payload: &[u8]) -> Result<Vec<u8>, std::io::Error> {
//         // Determine the buffer size based on payload size
//         let buffer_size = calculate_buffer_size(payload.len());
//
//         // Create an output buffer for the compressed data
//         let mut compressed_data = BytesMut::with_capacity(payload.len());
//
//         // Create a reusable buffer for intermediate compression chunks
//         let mut buffer = BytesMut::with_capacity(buffer_size);
//         buffer.resize(buffer_size, 0);
//
//         let mut offset = 0;
//
//         while offset < payload.len() {
//             let input = &payload[offset..];
//
//             // Determine flush strategy based on chunk position (intermediate or final)
//             let flush = if offset + input.len() == payload.len() {
//                 FlushCompress::Finish // Final chunk of data
//             } else {
//                 FlushCompress::Sync // Intermediate chunks
//             };
//
//             // Compress the input slice into the reusable buffer
//             let status = self
//                 .compressor
//                 .compress(input, &mut buffer, flush)?;
//
//             // Append compressed bytes directly to `compressed_data`
//             let bytes_written = self.compressor.total_out() as usize - compressed_data.len();
//             compressed_data.extend_from_slice(&buffer[..bytes_written]);
//
//             // Update the offset based on the amount of input consumed
//             let bytes_consumed = self.compressor.total_in() as usize - offset;
//             offset += bytes_consumed;
//
//             // Stop if the compression is complete
//             if Status::StreamEnd == status {
//                 break;
//             }
//
//             // Dynamically grow the buffer if necessary (adaptive sizing)
//             if buffer.len() < buffer_size {
//                 let new_size = cmp::min(buffer.len() * 2, 65536); // Cap growth at 64KB
//                 buffer.resize(new_size, 0);
//             }
//         }
//
//         Ok(compressed_data.to_vec())
//     }
// }
