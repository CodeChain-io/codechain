use codechain_types::{H256, Address};

type BlockNumber = u64;

/// A block header.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// Parent hash.
    pub parent_hash: H256,
    /// Block timestamp.
    pub timestamp: u64,
    /// Block number.
    pub number: BlockNumber,
    /// Block author.
    pub author: Address,

    /// Transactions root.
    pub transactions_root: H256,
    /// State root.
    pub state_root: H256,
}


