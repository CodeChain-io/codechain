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

use std::time::{SystemTime, UNIX_EPOCH};

use cmerkle::skewed_merkle_root;
use ctypes::{Bytes, H256};
use heapsize::HeapSizeOf;
use rlp::UntrustedRlp;
use unexpected::{Mismatch, OutOfBounds};

use super::super::blockchain::BlockProvider;
use super::super::client::BlockInfo;
use super::super::consensus::CodeChainEngine;
use super::super::error::{BlockError, Error};
use super::super::header::Header;
use super::super::parcel::{SignedParcel, UnverifiedParcel};
use super::super::types::BlockNumber;
use super::super::views::BlockView;

/// Preprocessed block data gathered in `verify_block_unordered` call
pub struct PreverifiedBlock {
    /// Populated block header
    pub header: Header,
    /// Populated block parcels
    pub parcels: Vec<SignedParcel>,
    /// Block bytes
    pub bytes: Bytes,
}

impl HeapSizeOf for PreverifiedBlock {
    fn heap_size_of_children(&self) -> usize {
        self.header.heap_size_of_children() + self.parcels.heap_size_of_children() + self.bytes.heap_size_of_children()
    }
}

/// Phase 1 quick block verification. Only does checks that are cheap. Operates on a single block
pub fn verify_block_basic(header: &Header, bytes: &[u8], engine: &CodeChainEngine) -> Result<(), Error> {
    verify_header_params(&header, engine)?;
    engine.verify_block_basic(&header)?;

    for t in UntrustedRlp::new(bytes).at(1)?.iter().map(|rlp| rlp.as_val::<UnverifiedParcel>()) {
        engine.verify_parcel_basic(&t?, &header)?;
    }
    Ok(())
}

/// Check basic header parameters.
pub fn verify_header_params(header: &Header, engine: &CodeChainEngine) -> Result<(), Error> {
    let expected_seal_fields = engine.seal_fields(header);
    if header.seal().len() != expected_seal_fields {
        return Err(From::from(BlockError::InvalidSealArity(Mismatch {
            expected: expected_seal_fields,
            found: header.seal().len(),
        })))
    }

    if header.number() >= From::from(BlockNumber::max_value()) {
        return Err(From::from(BlockError::RidiculousNumber(OutOfBounds {
            max: Some(From::from(BlockNumber::max_value())),
            min: None,
            found: header.number(),
        })))
    }
    let maximum_extra_data_size = engine.maximum_extra_data_size();
    if header.number() != 0 && header.extra_data().len() > maximum_extra_data_size {
        return Err(From::from(BlockError::ExtraDataOutOfBounds(OutOfBounds {
            min: None,
            max: Some(maximum_extra_data_size),
            found: header.extra_data().len(),
        })))
    }

    const ACCEPTABLE_DRIFT_SECS: u64 = 15;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let max_time = now.as_secs() + ACCEPTABLE_DRIFT_SECS;
    let invalid_threshold = max_time + ACCEPTABLE_DRIFT_SECS * 9;
    let timestamp = header.timestamp();

    if timestamp > invalid_threshold {
        return Err(From::from(BlockError::InvalidTimestamp(OutOfBounds {
            max: Some(max_time),
            min: None,
            found: timestamp,
        })))
    }

    if timestamp > max_time {
        return Err(From::from(BlockError::TemporarilyInvalid(OutOfBounds {
            max: Some(max_time),
            min: None,
            found: timestamp,
        })))
    }

    Ok(())
}

/// Verify block data against header: parcels root
fn verify_parcels_root(block: &[u8], parcels_root: &H256, parent_parcels_root: H256) -> Result<(), Error> {
    let block = UntrustedRlp::new(block);
    let parcel = block.at(1)?;
    let expected_root = skewed_merkle_root(parent_parcels_root, parcel.iter().map(|r| r.as_raw()));
    if &expected_root != parcels_root {
        return Err(From::from(BlockError::InvalidParcelsRoot(Mismatch {
            expected: expected_root.clone(),
            found: parcels_root.clone(),
        })))
    }
    Ok(())
}

/// Phase 2 verification. Perform costly checks such as parcel signatures and block nonce for ethash.
/// Still operates on a individual block
/// Returns a `PreverifiedBlock` structure populated with parcels
pub fn verify_block_unordered(
    header: Header,
    bytes: Bytes,
    engine: &CodeChainEngine,
    check_seal: bool,
) -> Result<PreverifiedBlock, Error> {
    if check_seal {
        engine.verify_block_unordered(&header)?;
    }
    // Verify parcels.
    let mut parcels = Vec::new();
    {
        let v = BlockView::new(&bytes);
        for t in v.parcels() {
            let signed_parcel = engine.verify_parcel_unordered(t, &header)?;
            parcels.push(signed_parcel);
        }
    }
    Ok(PreverifiedBlock {
        header,
        parcels,
        bytes,
    })
}

/// Parameters for full verification of block family
pub struct FullFamilyParams<'a, C: BlockInfo + 'a> {
    /// Serialized block bytes
    pub block_bytes: &'a [u8],

    /// Signed parcels
    pub parcels: &'a [SignedParcel],

    /// Block provider to use during verification
    pub block_provider: &'a BlockProvider,

    /// Engine client to use during verification
    pub client: &'a C,
}

/// Phase 3 verification. Check block information against parent and uncles.
pub fn verify_block_family<C: BlockInfo>(
    block: &[u8],
    header: &Header,
    parent: &Header,
    engine: &CodeChainEngine,
    do_full: Option<FullFamilyParams<C>>,
) -> Result<(), Error> {
    // TODO: verify timestamp
    verify_parent(&header, &parent)?;
    verify_parcels_root(block, header.parcels_root(), parent.parcels_root().clone())?;
    engine.verify_block_family(&header, &parent)?;

    let params = match do_full {
        Some(x) => x,
        None => return Ok(()),
    };

    for parcel in params.parcels {
        engine.machine().verify_parcel(parcel, header, params.client)?;
    }

    Ok(())
}

/// Check header parameters agains parent header.
fn verify_parent(header: &Header, parent: &Header) -> Result<(), Error> {
    if !header.parent_hash().is_zero() && &parent.hash() != header.parent_hash() {
        return Err(From::from(BlockError::InvalidParentHash(Mismatch {
            expected: parent.hash(),
            found: header.parent_hash().clone(),
        })))
    }
    if header.timestamp() <= parent.timestamp() {
        return Err(From::from(BlockError::InvalidTimestamp(OutOfBounds {
            max: None,
            min: Some(parent.timestamp() + 1),
            found: header.timestamp(),
        })))
    }
    if header.number() != parent.number() + 1 {
        return Err(From::from(BlockError::InvalidNumber(Mismatch {
            expected: parent.number() + 1,
            found: header.number(),
        })))
    }

    if header.number() == 0 {
        return Err(BlockError::RidiculousNumber(OutOfBounds {
            min: Some(1),
            max: None,
            found: header.number(),
        }).into())
    }

    Ok(())
}

/// Phase 4 verification. Check block information against parcel enactment results,
pub fn verify_block_final(expected: &Header, got: &Header) -> Result<(), Error> {
    if expected.state_root() != got.state_root() {
        return Err(From::from(BlockError::InvalidStateRoot(Mismatch {
            expected: expected.state_root().clone(),
            found: got.state_root().clone(),
        })))
    }
    if expected.invoices_root() != got.invoices_root() {
        return Err(From::from(BlockError::InvalidInvoicesRoot(Mismatch {
            expected: expected.invoices_root().clone(),
            found: got.invoices_root().clone(),
        })))
    }
    Ok(())
}
