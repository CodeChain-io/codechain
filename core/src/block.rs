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

use std::collections::HashSet;

use cbytes::Bytes;
use ccrypto::BLAKE_NULL_RLP;
use ctypes::{Address, H256};
use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};
use trie::TrieFactory;
use triehash::ordered_trie_root;

use super::consensus::CodeChainEngine;
use super::error::Error;
use super::header::{Header, Seal};
use super::invoice::Invoice;
use super::machine::{LiveBlock, Transactions};
use super::transaction::{UnverifiedTransaction, SignedTransaction, TransactionError};
use super::state::State;
use super::state_db::StateDB;

/// A block, encoded as it is on the block chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The header of this block
    pub header: Header,
    /// The transactions in this block.
    pub transactions: Vec<UnverifiedTransaction>,
}

impl Block {
    /// Get the RLP-encoding of the block with or without the seal.
    pub fn rlp_bytes(&self, seal: Seal) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.header.stream_rlp(&mut block_rlp, seal);
        block_rlp.append_list(&self.transactions);
        block_rlp.out()
    }
}

impl Decodable for Block {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.as_raw().len() != rlp.payload_info()?.total() {
            return Err(DecoderError::RlpIsTooBig);
        }
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        Ok(Block {
            header: rlp.val_at(0)?,
            transactions: rlp.list_at(1)?,
        })
    }
}

/// An internal type for a block's common elements.
pub struct ExecutedBlock {
    header: Header,
    state: State<StateDB>,
    transactions: Vec<SignedTransaction>,
    invoices: Vec<Invoice>,
    transactions_set: HashSet<H256>,
}

impl ExecutedBlock {
    fn new(state: State<StateDB>) -> ExecutedBlock {
        ExecutedBlock {
            header: Default::default(),
            state,
            transactions: Default::default(),
            invoices: Default::default(),
            transactions_set: Default::default(),
        }
    }

    /// Get mutable access to a state.
    pub fn state_mut(&mut self) -> &mut State<StateDB> {
        &mut self.state
    }
}

impl Transactions for ExecutedBlock {
    type Transaction = SignedTransaction;

    fn transactions(&self) -> &[SignedTransaction] {
        &self.transactions
    }
}

impl LiveBlock for ExecutedBlock {
    type Header = Header;

    fn header(&self) -> &Header {
        &self.header
    }
}

/// Block that is ready for transactions to be added.
pub struct OpenBlock<'x> {
    block: ExecutedBlock,
    engine: &'x CodeChainEngine,
}

impl<'x> OpenBlock<'x> {
    /// Create a new `OpenBlock` ready for transaction pushing.
    pub fn new(
        engine: &'x CodeChainEngine,
        trie_factory: TrieFactory,
        db: StateDB,
        parent: &Header,
        author: Address,
        extra_data: Bytes,
        is_epoch_begin: bool,
    ) -> Result<Self, Error> {
        let number = parent.number() + 1;
        let state = State::from_existing(db, *parent.state_root(), engine.machine().account_start_nonce(), trie_factory)?;
        let mut r = OpenBlock {
            block: ExecutedBlock::new(state),
            engine,
        };

        r.block.header.set_parent_hash(parent.hash());
        r.block.header.set_number(number);
        r.block.header.set_author(author);
        r.block.header.set_timestamp_now(parent.timestamp());
        r.block.header.set_extra_data(extra_data);
        r.block.header.note_dirty();

        engine.on_new_block(&mut r.block, is_epoch_begin)?;

        Ok(r)
    }

    /// Push a transaction into the block.
    pub fn push_transaction(&mut self, t: SignedTransaction, h: Option<H256>) -> Result<(), Error> {
        if self.block.transactions_set.contains(&t.hash()) {
            return Err(TransactionError::AlreadyImported.into());
        }

        let outcome = self.block.state.apply(&t)?;

        self.block.transactions_set.insert(h.unwrap_or_else(||t.hash()));
        self.block.transactions.push(t.into());
        self.block.invoices.push(outcome.invoice);
        Ok(())
    }

    /// Push transactions onto the block.
    pub fn push_transactions(&mut self, transactions: &[SignedTransaction]) -> Result<(), Error> {
        for t in transactions {
            self.push_transaction(t.clone(), None)?;
        }
        Ok(())
    }

    /// Turn this into a `LockedBlock`.
    pub fn close_and_lock(self) -> LockedBlock {
        let mut s = self;

        if let Err(e) = s.engine.on_close_block(&mut s.block) {
            warn!("Encountered error on closing the block: {}", e);
        }

        if let Err(e) = s.block.state.commit() {
            warn!("Encountered error on state commit: {}", e);
        }
        if s.block.header.transactions_root().is_zero() || s.block.header.transactions_root() == &BLAKE_NULL_RLP {
            s.block.header.set_transactions_root(ordered_trie_root(s.block.transactions.iter().map(|e| e.rlp_bytes())));
        }
        if s.block.header.invoices_root().is_zero() || s.block.header.invoices_root() == &BLAKE_NULL_RLP {
            s.block.header.set_invoices_root(ordered_trie_root(s.block.invoices.iter().map(|r| r.rlp_bytes())));
        }
        s.block.header.set_state_root(s.block.state.root().clone());

        LockedBlock {
            block: s.block,
        }
    }
}

/// Just like `ClosedBlock` except that we can't reopen it and it's faster.
pub struct LockedBlock {
    block: ExecutedBlock,
}

impl LockedBlock {
    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    /// This does check the validity of `seal` with the engine.
    /// Returns the `ClosedBlock` back again if the seal is no good.
    pub fn try_seal(
        self,
        engine: &CodeChainEngine,
        seal: Vec<Bytes>,
    ) -> Result<SealedBlock, (Error, LockedBlock)> {
        let mut s = self;
        s.block.header.set_seal(seal);

        // TODO: passing state context to avoid engines owning it?
        match engine.verify_local_seal(&s.block.header) {
            Err(e) => Err((e, s)),
            _ => Ok(SealedBlock { block: s.block }),
        }
    }

}

/// A block that has a valid seal.
///
/// The block's header has valid seal arguments. The block cannot be reversed into a `ClosedBlock` or `OpenBlock`.
pub struct SealedBlock {
    block: ExecutedBlock,
}

/// Trait for a object that is a `ExecutedBlock`.
pub trait IsBlock {
    /// Get the `ExecutedBlock` associated with this object.
    fn block(&self) -> &ExecutedBlock;

    /// Get the header associated with this object's block.
    fn header(&self) -> &Header { &self.block().header }

    /// Get all information on transactions in this block.
    fn transactions(&self) -> &[SignedTransaction] { &self.block().transactions }

    /// Get all information on receipts in this block.
    fn invoices(&self) -> &[Invoice] { &self.block().invoices }

    /// Get the final state associated with this object's block.
    fn state(&self) -> &State<StateDB> { &self.block().state }
}

impl IsBlock for ExecutedBlock {
    fn block(&self) -> &ExecutedBlock { self }
}

impl<'x> IsBlock for OpenBlock<'x> {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

impl<'x> IsBlock for LockedBlock {
    fn block(&self) -> &ExecutedBlock { &self.block }
}

/// Trait for a object that has a state database.
pub trait Drain {
    /// Drop this object and return the underlying database.
    fn drain(self) -> StateDB;
}

impl Drain for LockedBlock {
    fn drain(self) -> StateDB {
        self.block.state.drop().1
    }
}

impl Drain for SealedBlock {
    fn drain(self) -> StateDB {
        self.block.state.drop().1
    }
}

