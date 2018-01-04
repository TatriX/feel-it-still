extern crate tokio_io;
extern crate bytes;
extern crate byteorder;

#[macro_use]
extern crate log;

use tokio_io::codec::{Encoder, Decoder};
use bytes::{BytesMut, BufMut, LittleEndian};
use byteorder::ReadBytesExt;

use std::io::Cursor;
use std::io;

pub struct UintCodec {
    done: bool
}

impl UintCodec {
    pub fn new() -> UintCodec {
        UintCodec{done: false}
    }
}

impl Decoder for UintCodec {
    type Item = u64;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u64>> {
        if self.done {
            return Ok(None);
        }
        let size = std::mem::size_of::<u64>();
        if buf.len() >= size {
            self.done = true;
            let line = buf.split_to(size);
            let mut rdr = Cursor::new(&line);
            Ok(Some(rdr.read_u64::<LittleEndian>()?))
        } else {
            Err(io::Error::new(io::ErrorKind::TimedOut, "Timeout"))
        }
    }
}

impl Encoder for UintCodec {
    type Item = u64;
    type Error = io::Error;

     fn encode(&mut self, item: u64, buf: &mut BytesMut) -> io::Result<()> {
         debug!("Sending {}", item);
         buf.put_u64::<LittleEndian>(item);
         Ok(())
    }
}
