use codechain_types::{U256};

type Bytes = Vec<u8>;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    /// Transaction data.
    pub data: Bytes,
}
