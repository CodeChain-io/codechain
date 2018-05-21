use ccore::{LocalizedParcel, Transaction};
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

impl Parcel {
    pub fn from_localized(p: LocalizedParcel) -> Parcel {
        let sig = p.signature();
        Parcel {
            block_number: Some(p.block_number.clone()),
            block_hash: Some(p.block_hash.clone()),
            parcel_index: Some(p.parcel_index.clone()),
            nonce: p.nonce.clone(),
            fee: p.fee.clone(),
            transactions: p.transactions.clone(),
            network_id: p.network_id.clone(),
            hash: p.hash(),
            v: sig.v(),
            r: sig.r().into(),
            s: sig.s().into(),
        }
    }
}
