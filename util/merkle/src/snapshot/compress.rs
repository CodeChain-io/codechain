// Copyright 2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::io::{Cursor, Read, Write};

use rlp::{Rlp, RlpStream};

use super::chunk::{Chunk, RawChunk};
use super::error::{ChunkError, Error};
use super::CHUNK_MAX_NODES;

pub struct ChunkDecompressor<R> {
    read: R,
}

impl<R> ChunkDecompressor<R> {
    pub fn new(read: R) -> Self {
        ChunkDecompressor {
            read,
        }
    }
}

impl<'a> ChunkDecompressor<Cursor<&'a [u8]>> {
    fn from_slice(slice: &'a [u8]) -> Self {
        ChunkDecompressor::new(Cursor::new(slice))
    }
}

impl<R> ChunkDecompressor<R>
where
    R: Read + Clone,
{
    pub fn decompress(self) -> Result<RawChunk, Error> {
        let mut buf = Vec::new();

        let mut snappy = snap::Reader::new(self.read);
        snappy.read_to_end(&mut buf)?;

        let rlp = Rlp::new(&buf);
        let len = rlp.item_count()?;
        if len > CHUNK_MAX_NODES {
            return Err(ChunkError::TooBig.into())
        }

        Ok(RawChunk {
            nodes: rlp.as_list()?,
        })
    }
}

pub struct ChunkCompressor<W> {
    write: W,
}

impl<W> ChunkCompressor<W> {
    pub fn new(write: W) -> Self {
        ChunkCompressor {
            write,
        }
    }
}

impl<W> ChunkCompressor<W>
where
    W: Write,
{
    pub fn compress_chunk(self, chunk: &Chunk) -> Result<(), Error> {
        let mut rlp = RlpStream::new_list(chunk.terminal_nodes.len());
        for node in chunk.terminal_nodes.iter() {
            rlp.append(node);
        }
        let mut snappy = snap::Writer::new(self.write);
        snappy.write_all(rlp.as_raw())?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::snapshot::chunk::{Chunk, TerminalNode};

    #[test]
    fn test_compress_decompress() {
        let chunk = Chunk {
            root: Default::default(),
            terminal_nodes: vec![
                (TerminalNode {
                    path_slice: b"12345".to_vec(),
                    node_rlp: b"45678".to_vec(),
                }),
                (TerminalNode {
                    path_slice: b"56789".to_vec(),
                    node_rlp: b"123abc".to_vec(),
                }),
            ],
        };

        let mut buffer = Vec::new();
        ChunkCompressor::new(&mut buffer).compress_chunk(&chunk).unwrap();
        let decompressed = ChunkDecompressor::from_slice(&buffer).decompress().unwrap();

        assert_eq!(chunk.terminal_nodes, decompressed.nodes);
    }
}
