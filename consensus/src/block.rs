use super::transaction::Transaction;
use super::header::Header;

/// A block, encoded as it is on the block chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The header of this block
    pub header: Header,
    /// The transactions in this block.
    pub transactions: Vec<Transaction>,
}

