use crate::error::Error;
use crate::frame::Frame;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;

pub enum WriterKind {
    Client,
    Server,
}

pub struct Writer {
    write_half: WriteHalf<TcpStream>,
    kind: WriterKind,
}

impl Writer {
    pub fn new(write_half: WriteHalf<TcpStream>, kind: WriterKind) -> Self {
        Self { write_half, kind }
    }

    pub async fn write_frame(&mut self, frame: Frame) -> Result<(), Error> {
        match self.kind {
            WriterKind::Client => self.write_frame_client(frame).await,
            WriterKind::Server => self.write_frame_server(frame).await,
        }
    }

    pub async fn write_frame_server(&mut self, frame: Frame) -> Result<(), Error> {
        // The first byte of a websockets frame contains the final fragment bit, and the OpCode
        // in (frame.final_fragment as u8) << 7 we are doing a left bitwise shift, if final_fragment is true
        // it will be converted from 10000000 to 1
        // after that it will perform a bitwise OR operation with OpCode, so if Opcode is text(0x1)
        // the final result will be 10000001, which is 129 decimal
        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
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

    pub async fn write_frame_client(&mut self, frame: Frame) -> Result<(), Error> {
        let mut rng = StdRng::from_rng(rand::thread_rng());
        let mask = [
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
        ];

        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
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
