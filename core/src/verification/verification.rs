// Copyright 2018 Kodebox, Inc.
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

use cbytes::Bytes;
use heapsize::HeapSizeOf;

use super::super::blockchain::BlockProvider;
use super::super::client::BlockInfo;
use super::super::header::Header;
use super::super::transaction::SignedTransaction;

/// Preprocessed block data gathered in `verify_block_unordered` call
pub struct PreverifiedBlock {
    /// Populated block header
    pub header: Header,
    /// Populated block transactions
    pub transactions: Vec<SignedTransaction>,
    /// Block bytes
    pub bytes: Bytes,
}

impl HeapSizeOf for PreverifiedBlock {
    fn heap_size_of_children(&self) -> usize {
        self.header.heap_size_of_children()
            + self.transactions.heap_size_of_children()
            + self.bytes.heap_size_of_children()
    }
}

/// Parameters for full verification of block family
pub struct FullFamilyParams<'a, C: BlockInfo + 'a> {
    /// Serialized block bytes
    pub block_bytes: &'a [u8],

    /// Signed transactions
    pub transactions: &'a [SignedTransaction],

    /// Block provider to use during verification
    pub block_provider: &'a BlockProvider,

    /// Engine client to use during verification
    pub client: &'a C,
}

