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

use heapsize::HeapSizeOf;
use primitives::{H256, U256};

pub use self::blocks::Blocks;
pub use self::headers::Headers;

use crate::consensus::CodeChainEngine;
use crate::error::Error;
use crate::service::ClientIoMessage;

/// Something which can produce a hash and a parent hash.
pub trait BlockLike {
    /// Get the hash of this item.
    fn hash(&self) -> H256;

    /// Get the hash of this item's parent.
    fn parent_hash(&self) -> H256;

    /// Get the score of this item.
    fn score(&self) -> U256;
}

/// Defines transitions between stages of verification.
///
/// It starts with a fallible transformation from an "input" into the unverified item.
/// This consists of quick, simply done checks as well as extracting particular data.
///
/// Then, there is a `verify` function which performs more expensive checks and
/// produces the verified output.
///
/// For correctness, the hashes produced by each stage of the pipeline should be
/// consistent.
pub trait Kind: 'static + Sized + Send + Sync {
    /// The first stage: completely unverified.
    type Input: Sized + Send + BlockLike + HeapSizeOf;

    /// The second stage: partially verified.
    type Unverified: Sized + Send + BlockLike + HeapSizeOf;

    /// The third stage: completely verified.
    type Verified: Sized + Send + BlockLike + HeapSizeOf;

    /// Attempt to create the `Unverified` item from the input.
    fn create(input: Self::Input, engine: &CodeChainEngine) -> Result<Self::Unverified, Error>;

    /// Attempt to verify the `Unverified` item using the given engine.
    fn verify(
        unverified: Self::Unverified,
        engine: &CodeChainEngine,
        check_seal: bool,
    ) -> Result<Self::Verified, Error>;

    fn signal() -> ClientIoMessage;
}

/// Verification for headers.
pub mod headers {

    use primitives::{H256, U256};

    use super::super::super::verification::verify_header_params;
    use super::{BlockLike, Kind};
    use crate::consensus::CodeChainEngine;
    use crate::error::Error;
    use crate::header::Header;
    use crate::service::ClientIoMessage;


    impl BlockLike for Header {
        fn hash(&self) -> H256 {
            self.hash()
        }

        fn parent_hash(&self) -> H256 {
            *self.parent_hash()
        }

        fn score(&self) -> U256 {
            *self.score()
        }
    }

    /// A mode for verifying headers.
    pub struct Headers;

    impl Kind for Headers {
        type Input = Header;
        type Unverified = Header;
        type Verified = Header;

        fn create(input: Self::Input, engine: &CodeChainEngine) -> Result<Self::Unverified, Error> {
            // FIXME: this doesn't seem to match with full block verification
            verify_header_params(&input, engine).map(|_| input)
        }

        fn verify(un: Self::Unverified, engine: &CodeChainEngine, check_seal: bool) -> Result<Self::Verified, Error> {
            if check_seal {
                engine.verify_block_unordered(&un).map(|_| un)
            } else {
                Ok(un)
            }
        }

        fn signal() -> ClientIoMessage {
            ClientIoMessage::HeaderVerified
        }
    }
}

/// The blocks verification module.
pub mod blocks {
    use heapsize::HeapSizeOf;
    use primitives::{Bytes, H256, U256};

    use super::super::super::verification::{verify_block_basic, verify_block_unordered, PreverifiedBlock};
    use super::{BlockLike, Kind};
    use crate::consensus::CodeChainEngine;
    use crate::error::Error;
    use crate::header::Header;
    use crate::service::ClientIoMessage;

    /// A mode for verifying blocks.
    pub struct Blocks;

    impl Kind for Blocks {
        type Input = Unverified;
        type Unverified = Unverified;
        type Verified = PreverifiedBlock;

        fn create(input: Self::Input, engine: &CodeChainEngine) -> Result<Self::Unverified, Error> {
            match verify_block_basic(&input.header, &input.bytes, engine) {
                Ok(()) => Ok(input),
                Err(e) => {
                    cwarn!(CLIENT, "Stage 1 block verification failed for {}: {:?}", input.hash(), e);
                    Err(e)
                }
            }
        }

        fn verify(un: Self::Unverified, engine: &CodeChainEngine, check_seal: bool) -> Result<Self::Verified, Error> {
            let hash = un.hash();
            match verify_block_unordered(un.header, un.bytes, engine, check_seal) {
                Ok(verified) => Ok(verified),
                Err(e) => {
                    cwarn!(CLIENT, "Stage 2 block verification failed for {}: {:?}", hash, e);
                    Err(e)
                }
            }
        }

        fn signal() -> ClientIoMessage {
            ClientIoMessage::BlockVerified
        }
    }

    /// An unverified block.
    pub struct Unverified {
        header: Header,
        bytes: Bytes,
    }

    impl Unverified {
        /// Create an `Unverified` from raw bytes.
        pub fn new(bytes: Bytes) -> Self {
            use crate::views::BlockView;

            let header = BlockView::new(&bytes).header();
            Unverified {
                header,
                bytes,
            }
        }
    }

    impl HeapSizeOf for Unverified {
        fn heap_size_of_children(&self) -> usize {
            self.header.heap_size_of_children() + self.bytes.heap_size_of_children()
        }
    }

    impl BlockLike for Unverified {
        fn hash(&self) -> H256 {
            self.header.hash()
        }

        fn parent_hash(&self) -> H256 {
            *self.header.parent_hash()
        }

        fn score(&self) -> U256 {
            *self.header.score()
        }
    }

    impl BlockLike for PreverifiedBlock {
        fn hash(&self) -> H256 {
            self.header.hash()
        }

        fn parent_hash(&self) -> H256 {
            *self.header.parent_hash()
        }

        fn score(&self) -> U256 {
            *self.header.score()
        }
    }
}
