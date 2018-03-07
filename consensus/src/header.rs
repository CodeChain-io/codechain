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

use std::cmp;
use std::cell::RefCell;
use codechain_crypto::{blake256};
use codechain_types::{H256, U256, Address};
use super::Bytes;
use time::get_time;
use rlp::*;

type BlockNumber = u64;

/// Semantic boolean for when a seal/signature is included.
pub enum Seal {
    /// The seal/signature is included.
    With,
    /// The seal/signature is not included.
    Without,
}

/// A block header.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// Parent hash.
    parent_hash: H256,
    /// Block timestamp.
    timestamp: u64,
    /// Block number.
    number: BlockNumber,
    /// Block author.
    author: Address,

    /// Transactions root.
    transactions_root: H256,
    /// State root.
    state_root: H256,

    /// Vector of post-RLP-encoded fields.
    seal: Vec<Bytes>,

    /// The memoized hash of the RLP representation *including* the seal fields.
    hash: RefCell<Option<H256>>,
    /// The memoized hash of the RLP representation *without* the seal fields.
    bare_hash: RefCell<Option<H256>>,
}

impl Header {
    /// Get the parent_hash field of the header.
    pub fn parent_hash(&self) -> &H256 { &self.parent_hash }
    /// Get the timestamp field of the header.
    pub fn timestamp(&self) -> u64 { self.timestamp }
    /// Get the number field of the header.
    pub fn number(&self) -> BlockNumber { self.number }
    /// Get the author field of the header.
    pub fn author(&self) -> &Address { &self.author }

    /// Get the state root field of the header.
    pub fn state_root(&self) -> &H256 { &self.state_root }
    /// Get the transactions root field of the header.
    pub fn transactions_root(&self) -> &H256 { &self.transactions_root }
    /// Get the seal field of the header.
    pub fn seal(&self) -> &[Bytes] { &self.seal }

    /// Set the number field of the header.
    pub fn set_parent_hash(&mut self, a: H256) { self.parent_hash = a; self.note_dirty(); }
    /// Set the timestamp field of the header.
    pub fn set_timestamp(&mut self, a: u64) { self.timestamp = a; self.note_dirty(); }
    /// Set the timestamp field of the header to the current time.
    pub fn set_timestamp_now(&mut self, but_later_than: u64) { self.timestamp = cmp::max(get_time().sec as u64, but_later_than + 1); self.note_dirty(); }
    /// Set the number field of the header.
    pub fn set_number(&mut self, a: BlockNumber) { self.number = a; self.note_dirty(); }
    /// Set the author field of the header.
    pub fn set_author(&mut self, a: Address) { if a != self.author { self.author = a; self.note_dirty(); } }

    /// Set the state root field of the header.
    pub fn set_state_root(&mut self, a: H256) { self.state_root = a; self.note_dirty(); }
    /// Set the transactions root field of the header.
    pub fn set_transactions_root(&mut self, a: H256) { self.transactions_root = a; self.note_dirty() }
    /// Set the seal field of the header.
    pub fn set_seal(&mut self, a: Vec<Bytes>) { self.seal = a; self.note_dirty(); }

    /// Get the hash of this header (blake of the RLP).
    pub fn hash(&self) -> H256 {
        let mut hash = self.hash.borrow_mut();
        match &mut *hash {
            &mut Some(ref h) => h.clone(),
            hash @ &mut None => {
                let h = self.rlp_blake(Seal::With);
                *hash = Some(h.clone());
                h
            }
        }
    }

    /// Get the hash of the header excluding the seal
    pub fn bare_hash(&self) -> H256 {
        let mut hash = self.bare_hash.borrow_mut();
        match &mut *hash {
            &mut Some(ref h) => h.clone(),
            hash @ &mut None => {
                let h = self.rlp_blake(Seal::Without);
                *hash = Some(h.clone());
                h
            }
        }
    }

    /// Place this header into an RLP stream `s`, optionally `with_seal`.
    pub fn stream_rlp(&self, s: &mut RlpStream, with_seal: Seal) {
        s.begin_list(6 + match with_seal { Seal::With => self.seal.len(), _ => 0 });
        s.append(&self.parent_hash);
        s.append(&self.author);
        s.append(&self.state_root);
        s.append(&self.transactions_root);
        s.append(&self.number);
        s.append(&self.timestamp);
        if let Seal::With = with_seal {
            for b in &self.seal {
                s.append_raw(b, 1);
            }
        }
    }

    /// Get the RLP of this header, optionally `with_seal`.
    pub fn rlp(&self, with_seal: Seal) -> Bytes {
        let mut s = RlpStream::new();
        self.stream_rlp(&mut s, with_seal);
        s.out()
    }

    /// Note that some fields have changed. Resets the memoised hash.
    pub fn note_dirty(&self) {
        *self.hash.borrow_mut() = None;
        *self.bare_hash.borrow_mut() = None;
    }

    /// Get the Blake hash of this header, optionally `with_seal`.
    pub fn rlp_blake(&self, with_seal: Seal) -> H256 { blake256(&self.rlp(with_seal)) }
}

impl Decodable for Header {
    fn decode(r: &UntrustedRlp) -> Result<Self, DecoderError> {
        let mut blockheader = Header {
            parent_hash: r.val_at(0)?,
            author: r.val_at(1)?,
            state_root: r.val_at(2)?,
            transactions_root: r.val_at(3)?,
            number: r.val_at(4)?,
            timestamp: cmp::min(r.val_at::<U256>(5)?, u64::max_value().into()).as_u64(),
            seal: vec![],
            hash: RefCell::new(Some(blake256(r.as_raw()))),
            bare_hash: RefCell::new(None),
        };

        for i in 6..r.item_count()? {
            blockheader.seal.push(r.at(i)?.as_raw().to_vec())
        }

        Ok(blockheader)
    }
}

impl Encodable for Header {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.stream_rlp(s, Seal::With);
    }
}
