use crate::error::Error;
use crate::frame::Frame;
use crate::stream::SocketFlowStream;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::io::{AsyncWriteExt, WriteHalf};

pub enum WriterKind {
    Client,
    Server,
}

pub struct Writer {
    write_half: WriteHalf<SocketFlowStream>,
    kind: WriterKind,
}

impl Writer {
    pub fn new(write_half: WriteHalf<SocketFlowStream>, kind: WriterKind) -> Self {
        Self { write_half, kind }
    }

    pub async fn write_frame(&mut self, frame: Frame, set_rsv1: bool) -> Result<(), Error> {
        match self.kind {
            WriterKind::Client => self.write_frame_client(frame, set_rsv1).await,
            WriterKind::Server => self.write_frame_server(frame, set_rsv1).await,
        }
    }

    pub async fn write_frame_server(&mut self, frame: Frame, set_rsv1: bool) -> Result<(), Error> {
        // The first byte of a websockets frame contains the final fragment bit, and the OpCode
        // in (frame.final_fragment as u8) << 7 we are doing a left bitwise shift, if final_fragment is true
        // it will be converted from 10000000 to 1
        // after that it will perform a bitwise OR operation with OpCode, so if Opcode is text(0x1)
        // the final result will be 10000001, which is 129 decimal
        let mut first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();

        // Set the RSV1 bit if compression is enabled for this frame
        if set_rsv1 {
            first_byte |= 0x40; // Set RSV1
        }

        let payload_len = frame.payload.len();

        self.write_half.write_all(&[first_byte]).await?;

        // According to Websockets RFC, if the payload length is less or equal 125, it's written as a 8-bit unsigned integer
        // if it's between 126 and 65535, it's represented by additional 8 bytes.
        if payload_len <= 125 {
            self.write_half.write_all(&[payload_len as u8]).await?;
        } else if payload_len <= 65535 {
            self.write_half
                .write_all(&[126, (payload_len >> 8) as u8, payload_len as u8])
                .await?;
        } else {
            let bytes = payload_len.to_be_bytes();
            self.write_half
                .write_all(&[
                    127, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                    bytes[7],
                ])
                .await?;
        }

        self.write_half.write_all(&frame.payload).await?;

        Ok(())
    }

    // Method used for writing frames into the socket by clients
    pub async fn write_frame_client(&mut self, frame: Frame, set_rsv1: bool) -> Result<(), Error> {
        let mut rng = StdRng::from_rng(rand::thread_rng());
        // According to Websockets RFC, all frames sent from the client,
        // needs to have the payload masked
        let mask = [
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
        ];

        let mut first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();

        // Set the RSV1 bit if compression is enabled for this frame
        if set_rsv1 {
            first_byte |= 0x40; // Set RSV1
        }
        let payload_len = frame.payload.len();

        self.write_half.write_all(&[first_byte]).await?;

        if payload_len <= 125 {
            let length = 0b1000_0000 | payload_len as u8; // we set the MSB to 1 to signify that the payload is masked
            self.write_half.write_all(&[length]).await?; // write the masked length
            self.write_half.write_all(&mask).await?; // send the mask key
        } else if payload_len <= 65535 {
            self.write_half
                .write_all(&[
                    126 | 0b1000_0000,
                    (payload_len >> 8) as u8,
                    payload_len as u8,
                ])
                .await?;
            self.write_half.write_all(&mask).await?;
        } else {
            let bytes = payload_len.to_be_bytes();
            self.write_half
                .write_all(&[
                    127 | 0b1000_0000,
                    bytes[0],
                    bytes[1],
                    bytes[2],
                    bytes[3],
                    bytes[4],
                    bytes[5],
                    bytes[6],
                    bytes[7],
                ])
                .await?;
            self.write_half.write_all(&mask).await?;
        }

        let mut masked_payload: Vec<u8> = Vec::with_capacity(frame.payload.len());
        for (i, &byte) in frame.payload.iter().enumerate() {
            masked_payload.push(byte ^ mask[i % 4]);
        }

        self.write_half.write_all(&masked_payload).await?;

        Ok(())
    }
}
