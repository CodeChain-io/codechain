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

use ckey::Address;
use primitives::{H256, U256};

/// A header. This contains important metadata about the block, as well as a
/// "seal" that indicates validity to a consensus engine.
pub trait Header {
    /// Cryptographic hash of the header, excluding the seal.
    fn bare_hash(&self) -> H256;

    /// Cryptographic hash of the header, including the seal.
    fn hash(&self) -> H256;

    /// Get a reference to the seal fields.
    fn seal(&self) -> &[Vec<u8>];

    /// The author of the header.
    fn author(&self) -> &Address;

    /// The number of the header.
    fn number(&self) -> u64;
}

/// A header with an associated score (difficulty in PoW terms)
pub trait ScoredHeader: Header {
    /// Get the score of this header.
    fn score(&self) -> &U256;

    /// Set the score of this header.
    fn set_score(&mut self, score: U256);
}

/// A "live" block is one which is in the process of the transition.
/// The state of this block can be mutated by arbitrary rules of the
/// state transition function.
pub trait LiveBlock: 'static {
    /// The block header type;
    type Header: Header;

    /// Get a reference to the header.
    fn header(&self) -> &Self::Header;
}

/// Trait for blocks which have a transaction type.
pub trait Transactions: LiveBlock {
    /// The transaction type.
    type Transaction;

    /// Get a reference to the transactions in this block.
    fn transactions(&self) -> &[Self::Transaction];
}

/// Generalization of types surrounding blockchain-suitable state machines.
pub trait Machine: Send + Sync {
    /// The block header type.
    type Header: Header;
    /// The live block type.
    type LiveBlock: LiveBlock<Header = Self::Header>;
    /// A handle to a blockchain client for this machine.
    type EngineClient: ?Sized;

    /// Errors which can occur when querying or interacting with the machine.
    type Error;
}

pub trait WithBalances: Machine {
    /// Get the balance, in base units, associated with an account.
    /// Extracts data from the live block.
    fn balance(&self, live: &Self::LiveBlock, address: &Address) -> Result<u64, Self::Error>;

    /// Increment the balance of an account in the state of the live block.
    fn add_balance(&self, live: &mut Self::LiveBlock, address: &Address, quantity: u64) -> Result<(), Self::Error>;
}
