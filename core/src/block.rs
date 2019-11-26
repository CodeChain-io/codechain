// Copyright 2018-2019 Kodebox, Inc.
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

use ccrypto::BLAKE_NULL_RLP;
use ckey::Address;
use cmerkle::skewed_merkle_root;
use cstate::{FindActionHandler, StateDB, StateError, StateWithCache, TopLevelState};
use ctypes::errors::HistoryError;
use ctypes::header::{Header, Seal};
use ctypes::util::unexpected::Mismatch;
use ctypes::{BlockNumber, CommonParams, TxHash};
use cvm::ChainTimeInfo;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

use super::invoice::Invoice;
use crate::client::{EngineInfo, TermInfo};
use crate::consensus::CodeChainEngine;
use crate::error::{BlockError, Error};
use crate::transaction::{SignedTransaction, UnverifiedTransaction};
use crate::BlockId;

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
    pub fn rlp_bytes(&self, seal: &Seal) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.header.stream_rlp(&mut block_rlp, seal);
        block_rlp.append_list(&self.transactions);
        block_rlp.out()
    }
}

impl Decodable for Block {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let got = rlp.as_raw().len();
        let expected = rlp.payload_info()?.total();
        if got > expected {
            return Err(DecoderError::RlpIsTooBig {
                expected,
                got,
            })
        }
        if got < expected {
            return Err(DecoderError::RlpIsTooShort {
                expected,
                got,
            })
        }
        let item_count = rlp.item_count()?;
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen {
                expected: 2,
                got: item_count,
            })
        }
        Ok(Block {
            header: rlp.val_at(0)?,
            transactions: rlp.list_at(1)?,
        })
    }
}

/// An internal type for a block's common elements.
#[derive(Clone)]
pub struct ExecutedBlock {
    header: Header,
    state: TopLevelState,
    transactions: Vec<SignedTransaction>,
    invoices: Vec<Invoice>,
    transactions_set: HashSet<TxHash>,
}

impl ExecutedBlock {
    fn new(state: TopLevelState, parent: &Header) -> ExecutedBlock {
        ExecutedBlock {
            header: parent.generate_child(),
            state,
            transactions: Default::default(),
            invoices: Default::default(),
            transactions_set: Default::default(),
        }
    }

    /// Get mutable access to a state.
    pub fn state_mut(&mut self) -> &mut TopLevelState {
        &mut self.state
    }

    pub fn transactions(&self) -> &[SignedTransaction] {
        &self.transactions
    }

    pub fn header(&self) -> &Header {
        &self.header
    }
}

/// Block that is ready for transactions to be added.
pub struct OpenBlock<'x> {
    block: ExecutedBlock,
    engine: &'x dyn CodeChainEngine,
}

impl<'x> OpenBlock<'x> {
    /// Create a new `OpenBlock` ready for transaction pushing.
    pub fn try_new(
        engine: &'x dyn CodeChainEngine,
        db: StateDB,
        parent: &Header,
        author: Address,
        extra_data: Bytes,
    ) -> Result<Self, Error> {
        let state = TopLevelState::from_existing(db, *parent.state_root()).map_err(StateError::from)?;
        let mut r = OpenBlock {
            block: ExecutedBlock::new(state, parent),
            engine,
        };

        r.block.header.set_author(author);
        r.block.header.set_extra_data(extra_data);
        r.block.header.note_dirty();

        engine.machine().populate_from_parent(&mut r.block.header, parent);
        engine.populate_from_parent(&mut r.block.header, parent);

        Ok(r)
    }

    /// Push a transaction into the block.
    pub fn push_transaction<C: ChainTimeInfo + FindActionHandler>(
        &mut self,
        tx: SignedTransaction,
        h: Option<TxHash>,
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> Result<(), Error> {
        if self.block.transactions_set.contains(&tx.hash()) {
            return Err(HistoryError::TransactionAlreadyImported.into())
        }

        let hash = tx.hash();
        let tracker = tx.tracker();
        let error = match self.block.state.apply(
            &tx,
            &hash,
            &tx.signer_public(),
            client,
            parent_block_number,
            parent_block_timestamp,
            self.block.header.timestamp(),
        ) {
            Ok(()) => {
                self.block.transactions_set.insert(h.unwrap_or(hash));
                self.block.transactions.push(tx);
                None
            }
            Err(err) => Some(err),
        };
        self.block.invoices.push(Invoice {
            hash,
            tracker,
            error: error.clone().map(|err| err.to_string()),
        });

        match error {
            None => Ok(()),
            Some(err) => Err(err.into()),
        }
    }

    /// Push transactions onto the block.
    pub fn push_transactions<C: ChainTimeInfo + FindActionHandler>(
        &mut self,
        transactions: &[SignedTransaction],
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> Result<(), Error> {
        for tx in transactions {
            self.push_transaction(tx.clone(), None, client, parent_block_number, parent_block_timestamp)?;
        }
        Ok(())
    }

    /// Populate self from a header.
    fn populate_from(&mut self, header: &Header) {
        self.block.header.set_score(*header.score());
        self.block.header.set_timestamp(header.timestamp());
        self.block.header.set_author(*header.author());
        self.block.header.set_transactions_root(*header.transactions_root());
        self.block.header.set_extra_data(header.extra_data().clone());
        self.block.header.set_seal(header.seal().to_vec());
    }

    /// Turn this into a `ClosedBlock`.
    pub fn close(
        mut self,
        parent_header: &Header,
        term_common_params: Option<&CommonParams>,
    ) -> Result<ClosedBlock, Error> {
        let unclosed_state = self.block.state.clone();

        if let Err(e) = self.engine.on_close_block(&mut self.block, term_common_params) {
            warn!("Encountered error on closing the block: {}", e);
            return Err(e)
        }
        let header = self.block.header().clone();
        for handler in self.engine.action_handlers() {
            handler.on_close_block(self.block.state_mut(), &header).map_err(|e| {
                warn!("Encountered error in {}::on_close_block", handler.name());
                e
            })?;
        }

        let state_root = self.block.state.commit().map_err(|e| {
            warn!("Encountered error on state commit: {}", e);
            e
        })?;
        let parent_transactions_root = parent_header.transactions_root();
        self.block.header.set_transactions_root(skewed_merkle_root(
            *parent_transactions_root,
            self.block.transactions.iter().map(Encodable::rlp_bytes),
        ));
        self.block.header.set_state_root(state_root);

        Ok(ClosedBlock {
            block: self.block,
            unclosed_state,
        })
    }

    /// Turn this into a `LockedBlock`.
    pub fn close_and_lock(
        mut self,
        parent_header: &Header,
        term_common_params: Option<&CommonParams>,
    ) -> Result<LockedBlock, Error> {
        if let Err(e) = self.engine.on_close_block(&mut self.block, term_common_params) {
            warn!("Encountered error on closing the block: {}", e);
            return Err(e)
        }
        let header = self.block.header().clone();
        for handler in self.engine.action_handlers() {
            handler.on_close_block(self.block.state_mut(), &header).map_err(|e| {
                warn!("Encountered error in {}::on_close_block", handler.name());
                e
            })?;
        }

        let state_root = self.block.state.commit().map_err(|e| {
            warn!("Encountered error on state commit: {}", e);
            e
        })?;
        let parent_transactions_root = parent_header.transactions_root();
        if self.block.header.transactions_root().is_zero() || self.block.header.transactions_root() == &BLAKE_NULL_RLP {
            self.block.header.set_transactions_root(skewed_merkle_root(
                *parent_transactions_root,
                self.block.transactions.iter().map(Encodable::rlp_bytes),
            ));
        }
        debug_assert_eq!(
            self.block.header.transactions_root(),
            &skewed_merkle_root(*parent_transactions_root, self.block.transactions.iter().map(Encodable::rlp_bytes),)
        );
        self.block.header.set_state_root(state_root);

        Ok(LockedBlock {
            block: self.block,
        })
    }

    /// Alter the timestamp of the block.
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.block.header.set_timestamp(timestamp);
    }

    /// Provide a valid seal
    ///
    /// NOTE: This does not check the validity of `seal` with the engine.
    pub fn seal(&mut self, engine: &dyn CodeChainEngine, seal: Vec<Bytes>) -> Result<(), BlockError> {
        let expected_seal_fields = engine.seal_fields(self.header());
        if seal.len() != expected_seal_fields {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal.len(),
            }))
        }
        self.block.header.set_seal(seal);
        Ok(())
    }

    pub fn inner_mut(&mut self) -> &mut ExecutedBlock {
        &mut self.block
    }
}

/// Just like `OpenBlock`, except that we've applied `Engine::on_close_block`, finished up the non-seal header fields.
///
/// There is no function available to push a transaction.
#[derive(Clone)]
pub struct ClosedBlock {
    block: ExecutedBlock,
    unclosed_state: TopLevelState,
}

impl ClosedBlock {
    /// Get the hash of the header without seal arguments.
    pub fn hash(&self) -> H256 {
        self.header().rlp_blake(&Seal::Without)
    }

    /// Turn this into a `LockedBlock`, unable to be reopened again.
    pub fn lock(self) -> LockedBlock {
        LockedBlock {
            block: self.block,
        }
    }

    /// Given an engine reference, reopen the `ClosedBlock` into an `OpenBlock`.
    pub fn reopen(self, engine: &dyn CodeChainEngine) -> OpenBlock {
        // revert rewards (i.e. set state back at last transaction's state).
        let mut block = self.block;
        block.state = self.unclosed_state;
        OpenBlock {
            block,
            engine,
        }
    }
}

/// Just like `ClosedBlock` except that we can't reopen it and it's faster.
pub struct LockedBlock {
    block: ExecutedBlock,
}

impl LockedBlock {
    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    ///
    /// NOTE: This does not check the validity of `seal` with the engine.
    pub fn seal(mut self, engine: &dyn CodeChainEngine, seal: Vec<Bytes>) -> Result<SealedBlock, BlockError> {
        let expected_seal_fields = engine.seal_fields(self.header());
        if seal.len() != expected_seal_fields {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal.len(),
            }))
        }
        self.block.header.set_seal(seal);
        Ok(SealedBlock {
            block: self.block,
        })
    }

    /// Provide a valid seal in order to turn this into a `SealedBlock`.
    /// This does check the validity of `seal` with the engine.
    /// Returns the `ClosedBlock` back again if the seal is no good.
    pub fn try_seal(
        mut self,
        engine: &dyn CodeChainEngine,
        seal: Vec<Bytes>,
    ) -> Result<SealedBlock, (Error, LockedBlock)> {
        self.block.header.set_seal(seal);

        // TODO: passing state context to avoid engines owning it?
        match engine.verify_local_seal(&self.block.header) {
            Err(e) => Err((e, self)),
            _ => Ok(SealedBlock {
                block: self.block,
            }),
        }
    }

    pub fn already_sealed(self) -> SealedBlock {
        SealedBlock {
            block: self.block,
        }
    }
}

/// A block that has a valid seal.
///
/// The block's header has valid seal arguments. The block cannot be reversed into a `ClosedBlock` or `OpenBlock`.
#[derive(Clone)]
pub struct SealedBlock {
    block: ExecutedBlock,
}

impl SealedBlock {
    /// Get the RLP-encoding of the block.
    pub fn rlp_bytes(&self) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.block.header.stream_rlp(&mut block_rlp, &Seal::With);
        block_rlp.append_list(&self.block.transactions);
        block_rlp.out()
    }
}

/// Trait for a object that is a `ExecutedBlock`.
pub trait IsBlock {
    /// Get the `ExecutedBlock` associated with this object.
    fn block(&self) -> &ExecutedBlock;

    /// Get the base `Block` object associated with this.
    fn to_base(&self) -> Block {
        Block {
            header: self.header().clone(),
            transactions: self.transactions().iter().cloned().map(Into::into).collect(),
        }
    }

    /// Get the header associated with this object's block.
    fn header(&self) -> &Header {
        &self.block().header
    }

    /// Get all information on transactions in this block.
    fn transactions(&self) -> &[SignedTransaction] {
        &self.block().transactions
    }

    /// Get all information on receipts in this block.
    fn invoices(&self) -> &[Invoice] {
        &self.block().invoices
    }

    /// Get the final state associated with this object's block.
    fn state(&self) -> &TopLevelState {
        &self.block().state
    }
}

impl IsBlock for ExecutedBlock {
    fn block(&self) -> &ExecutedBlock {
        self
    }
}

impl<'x> IsBlock for OpenBlock<'x> {
    fn block(&self) -> &ExecutedBlock {
        &self.block
    }
}

impl<'x> IsBlock for ClosedBlock {
    fn block(&self) -> &ExecutedBlock {
        &self.block
    }
}

impl<'x> IsBlock for LockedBlock {
    fn block(&self) -> &ExecutedBlock {
        &self.block
    }
}

impl IsBlock for SealedBlock {
    fn block(&self) -> &ExecutedBlock {
        &self.block
    }
}

/// Enact the block given by block header, transactions and uncles
pub fn enact<C: ChainTimeInfo + EngineInfo + FindActionHandler + TermInfo>(
    header: &Header,
    transactions: &[SignedTransaction],
    engine: &dyn CodeChainEngine,
    client: &C,
    db: StateDB,
    parent: &Header,
) -> Result<LockedBlock, Error> {
    let mut b = OpenBlock::try_new(engine, db, parent, Address::default(), vec![])?;

    b.populate_from(header);
    engine.on_open_block(b.inner_mut())?;
    b.push_transactions(transactions, client, parent.number(), parent.timestamp())?;

    let term_common_params = client.term_common_params(BlockId::Hash(*header.parent_hash()));
    b.close_and_lock(parent, term_common_params.as_ref())
}

#[cfg(test)]
mod tests {
    use ctypes::CommonParams;

    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    #[test]
    fn open_block() {
        let scheme = Scheme::new_test();
        let genesis_header = scheme.genesis_header();
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let b = OpenBlock::try_new(&*scheme.engine, db, &genesis_header, Address::default(), vec![]).unwrap();
        let term_common_params = CommonParams::default_for_test();
        let b = b.close_and_lock(&genesis_header, Some(&term_common_params)).unwrap();
        let _ = b.seal(&*scheme.engine, vec![]);
    }
}
