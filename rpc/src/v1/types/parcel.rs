use ccore::{LocalizedParcel, SignedParcel, Transaction};
use ctypes::{H256, U256};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcel {
    pub block_number: Option<u64>,
    pub block_hash: Option<H256>,
    pub parcel_index: Option<usize>,
    pub nonce: U256,
    pub fee: U256,
    pub transactions: Vec<Transaction>,
    pub network_id: u64,
    pub hash: H256,
    pub v: u8,
    pub r: U256,
    pub s: U256,
}

impl From<LocalizedParcel> for Parcel {
    fn from(p: LocalizedParcel) -> Self {
        let sig = p.signature();
        Self {
            block_number: Some(p.block_number),
            block_hash: Some(p.block_hash),
            parcel_index: Some(p.parcel_index),
            nonce: p.nonce,
            fee: p.fee,
            transactions: p.transactions.clone(),
            network_id: p.network_id,
            hash: p.hash(),
            v: sig.v(),
            r: sig.r().into(),
            s: sig.s().into(),
        }
    }
}

impl From<SignedParcel> for Parcel {
    fn from(p: SignedParcel) -> Self {
        let sig = p.signature();
        Self {
            block_number: None,
            block_hash: None,
            parcel_index: None,
            nonce: p.nonce,
            fee: p.fee,
            transactions: p.transactions.clone(),
            network_id: p.network_id,
            hash: p.hash(),
            v: sig.v(),
            r: sig.r().into(),
            s: sig.s().into(),
        }
    }
}
