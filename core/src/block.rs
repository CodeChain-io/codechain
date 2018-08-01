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

use ccrypto::BLAKE_NULL_RLP;
use ckey::Address;
use cmerkle::skewed_merkle_root;
use cmerkle::TrieFactory;
use cstate::{StateDB, StateError, StateWithCache, TopLevelState};
use ctypes::invoice::{Invoice, ParcelInvoice};
use ctypes::machine::{LiveBlock, Parcels};
use ctypes::parcel::{Error as ParcelError, Outcome as ParcelOutcome};
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use unexpected::Mismatch;

use super::consensus::CodeChainEngine;
use super::error::{BlockError, Error};
use super::header::{Header, Seal};
use super::parcel::{SignedParcel, UnverifiedParcel};

/// A block, encoded as it is on the block chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The header of this block
    pub header: Header,
    /// The parcels in this block.
    pub parcels: Vec<UnverifiedParcel>,
}

impl Block {
    /// Get the RLP-encoding of the block with or without the seal.
    pub fn rlp_bytes(&self, seal: Seal) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.header.stream_rlp(&mut block_rlp, seal);
        block_rlp.append_list(&self.parcels);
        block_rlp.out()
    }
}

impl Decodable for Block {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.as_raw().len() != rlp.payload_info()?.total() {
            return Err(DecoderError::RlpIsTooBig)
        }
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        Ok(Block {
            header: rlp.val_at(0)?,
            parcels: rlp.list_at(1)?,
        })
    }
}

/// An internal type for a block's common elements.
#[derive(Clone)]
pub struct ExecutedBlock {
    header: Header,
    state: TopLevelState,
    parcels: Vec<SignedParcel>,
    invoices: Vec<ParcelInvoice>,
    parcels_set: HashSet<H256>,
}

impl ExecutedBlock {
    fn new(state: TopLevelState) -> ExecutedBlock {
        ExecutedBlock {
            header: Default::default(),
            state,
            parcels: Default::default(),
            invoices: Default::default(),
            parcels_set: Default::default(),
        }
    }

    /// Get mutable access to a state.
    pub fn state_mut(&mut self) -> &mut TopLevelState {
        &mut self.state
    }
}

impl Parcels for ExecutedBlock {
    type Parcel = SignedParcel;

    fn parcels(&self) -> &[SignedParcel] {
        &self.parcels
    }
}

impl LiveBlock for ExecutedBlock {
    type Header = Header;

    fn header(&self) -> &Header {
        &self.header
    }
}

/// Block that is ready for parcels to be added.
pub struct OpenBlock<'x> {
    block: ExecutedBlock,
    engine: &'x CodeChainEngine,
}

impl<'x> OpenBlock<'x> {
    /// Create a new `OpenBlock` ready for parcel pushing.
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
        let state = TopLevelState::from_existing(db, *parent.state_root(), trie_factory).map_err(StateError::from)?;
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

        engine.machine().populate_from_parent(&mut r.block.header, parent);
        engine.populate_from_parent(&mut r.block.header, parent);

        engine.on_new_block(&mut r.block, is_epoch_begin)?;

        Ok(r)
    }

    /// Push a parcel into the block.
    pub fn push_parcel(&mut self, parcel: SignedParcel, h: Option<H256>) -> Result<(), Error> {
        if self.block.parcels_set.contains(&parcel.hash()) {
            return Err(StateError::Parcel(ParcelError::ParcelAlreadyImported).into())
        }

        let outcomes = self.block.state.apply(&parcel, parcel.sender(), &parcel.public_key())?;

        self.block.parcels_set.insert(h.unwrap_or_else(|| parcel.hash()));
        self.block.parcels.push(parcel.into());
        match outcomes {
            ParcelOutcome::Single {
                invoice,
                ..
            } => {
                self.block.invoices.push(ParcelInvoice::Single(invoice));
            }
            ParcelOutcome::Transactions(invoices) => {
                self.block
                    .invoices
                    .push(invoices.into_iter().map(|outcome| outcome.invoice).collect::<Vec<Invoice>>().into());
            }
        }
        Ok(())
    }

    /// Push parcels onto the block.
    pub fn push_parcels(&mut self, parcels: &[SignedParcel]) -> Result<(), Error> {
        for parcel in parcels {
            self.push_parcel(parcel.clone(), None)?;
        }
        Ok(())
    }

    /// Populate self from a header.
    fn populate_from(&mut self, header: &Header) {
        self.block.header.set_score(*header.score());
        self.block.header.set_timestamp(header.timestamp());
        self.block.header.set_author(*header.author());
        self.block.header.set_parcels_root(*header.parcels_root());
        self.block.header.set_extra_data(header.extra_data().clone());
    }

    /// Turn this into a `ClosedBlock`.
    pub fn close(mut self, parent_parcels_root: H256, parent_invoices_root: H256) -> ClosedBlock {
        let unclosed_state = self.block.state.clone();

        if let Err(e) = self.engine.on_close_block(&mut self.block) {
            warn!("Encountered error on closing the block: {}", e);
        }

        if let Err(e) = self.block.state.commit() {
            warn!("Encountered error on state commit: {}", e);
        }
        self.block.header.set_parcels_root(skewed_merkle_root(
            parent_parcels_root,
            self.block.parcels.iter().map(|e| e.rlp_bytes()),
        ));
        self.block.header.set_state_root(self.block.state.root().clone());
        self.block.header.set_invoices_root(skewed_merkle_root(
            parent_invoices_root,
            self.block.invoices.iter().flat_map(|invoices| invoices.iter().map(|invoice| invoice.rlp_bytes())),
        ));

        ClosedBlock {
            block: self.block,
            unclosed_state,
        }
    }

    /// Turn this into a `LockedBlock`.
    pub fn close_and_lock(mut self, parent_parcels_root: H256, parent_invoices_root: H256) -> LockedBlock {
        if let Err(e) = self.engine.on_close_block(&mut self.block) {
            warn!("Encountered error on closing the block: {}", e);
        }

        if let Err(e) = self.block.state.commit() {
            warn!("Encountered error on state commit: {}", e);
        }
        if self.block.header.parcels_root().is_zero() || self.block.header.parcels_root() == &BLAKE_NULL_RLP {
            self.block.header.set_parcels_root(skewed_merkle_root(
                parent_parcels_root,
                self.block.parcels.iter().map(|e| e.rlp_bytes()),
            ));
        }
        if self.block.header.invoices_root().is_zero() || self.block.header.invoices_root() == &BLAKE_NULL_RLP {
            self.block.header.set_invoices_root(skewed_merkle_root(
                parent_invoices_root,
                self.block.invoices.iter().flat_map(|invoices| invoices.iter().map(|invoice| invoice.rlp_bytes())),
            ));
        }
        self.block.header.set_state_root(self.block.state.root().clone());

        LockedBlock {
            block: self.block,
        }
    }

    /// Alter the timestamp of the block.
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.block.header.set_timestamp(timestamp);
    }
}

/// Just like `OpenBlock`, except that we've applied `Engine::on_close_block`, finished up the non-seal header fields.
///
/// There is no function available to push a parcel.
#[derive(Clone)]
pub struct ClosedBlock {
    block: ExecutedBlock,
    unclosed_state: TopLevelState,
}

impl ClosedBlock {
    /// Get the hash of the header without seal arguments.
    pub fn hash(&self) -> H256 {
        self.header().rlp_blake(Seal::Without)
    }

    /// Turn this into a `LockedBlock`, unable to be reopened again.
    pub fn lock(self) -> LockedBlock {
        LockedBlock {
            block: self.block,
        }
    }

    /// Given an engine reference, reopen the `ClosedBlock` into an `OpenBlock`.
    pub fn reopen(self, engine: &CodeChainEngine) -> OpenBlock {
        // revert rewards (i.e. set state back at last parcel's state).
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
    pub fn seal(mut self, engine: &CodeChainEngine, seal: Vec<Bytes>) -> Result<SealedBlock, BlockError> {
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
    pub fn try_seal(mut self, engine: &CodeChainEngine, seal: Vec<Bytes>) -> Result<SealedBlock, (Error, LockedBlock)> {
        self.block.header.set_seal(seal);

        // TODO: passing state context to avoid engines owning it?
        match engine.verify_local_seal(&self.block.header) {
            Err(e) => Err((e, self)),
            _ => Ok(SealedBlock {
                block: self.block,
            }),
        }
    }
}

/// A block that has a valid seal.
///
/// The block's header has valid seal arguments. The block cannot be reversed into a `ClosedBlock` or `OpenBlock`.
pub struct SealedBlock {
    block: ExecutedBlock,
}

impl SealedBlock {
    /// Get the RLP-encoding of the block.
    pub fn rlp_bytes(&self) -> Bytes {
        let mut block_rlp = RlpStream::new_list(2);
        self.block.header.stream_rlp(&mut block_rlp, Seal::With);
        block_rlp.append_list(&self.block.parcels);
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
            parcels: self.parcels().iter().cloned().map(Into::into).collect(),
        }
    }

    /// Get the header associated with this object's block.
    fn header(&self) -> &Header {
        &self.block().header
    }

    /// Get all information on parcels in this block.
    fn parcels(&self) -> &[SignedParcel] {
        &self.block().parcels
    }

    /// Get all information on receipts in this block.
    fn invoices(&self) -> &[ParcelInvoice] {
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

/// Enact the block given by block header, parcels and uncles
pub fn enact(
    header: &Header,
    parcels: &[SignedParcel],
    engine: &CodeChainEngine,
    db: StateDB,
    parent: &Header,
    trie_factory: TrieFactory,
    is_epoch_begin: bool,
) -> Result<LockedBlock, Error> {
    let mut b = OpenBlock::new(engine, trie_factory, db, parent, Address::new(), vec![], is_epoch_begin)?;

    b.populate_from(header);
    b.push_parcels(parcels)?;

    Ok(b.close_and_lock(parent.parcels_root().clone(), parent.invoices_root().clone()))
}

#[cfg(test)]
mod tests {
    use ckey::Address;

    use super::super::spec::Spec;
    use super::super::tests::helpers::get_temp_state_db;
    use super::OpenBlock;

    #[test]
    fn open_block() {
        let spec = Spec::new_test();
        let genesis_header = spec.genesis_header();
        let db = spec.ensure_genesis_state(get_temp_state_db(), &Default::default()).unwrap();
        let b = OpenBlock::new(&*spec.engine, Default::default(), db, &genesis_header, Address::zero(), vec![], false)
            .unwrap();
        let parent_parcels_root = genesis_header.parcels_root().clone();
        let parent_invoices_root = genesis_header.invoices_root().clone();
        let b = b.close_and_lock(parent_parcels_root, parent_invoices_root);
        let _ = b.seal(&*spec.engine, vec![]);
    }
}
