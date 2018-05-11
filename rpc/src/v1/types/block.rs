use ccore::{Action, Block as CoreBlock};
use ctypes::{H160, H256, U256};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcel {
    nonce: U256,
    fee: U256,
    action: Action,
    network_id: u64,
    hash: H256,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    parent_hash: H256,
    timestamp: u64,
    number: u64,
    author: H160,

    extra_data: Vec<u8>,

    parcelss_root: H256,
    state_root: H256,
    invoices_root: H256,

    score: U256,
    seal: Vec<Vec<u8>>,

    hash: H256,
    parcels: Vec<Parcel>,
}

impl From<CoreBlock> for Block {
    fn from(block: CoreBlock) -> Self {
        Block {
            parent_hash: block.header.parent_hash().clone(),
            timestamp: block.header.timestamp(),
            number: block.header.number(),
            author: block.header.author().clone(),

            extra_data: block.header.extra_data().clone(),

            parcelss_root: block.header.parcels_root().clone(),
            state_root: block.header.state_root().clone(),
            invoices_root: block.header.invoices_root().clone(),

            score: block.header.score().clone(),
            seal: block.header.seal().clone().to_vec(),

            hash: block.header.hash(),
            parcels: block
                .parcels
                .into_iter()
                .map(|unverified| Parcel {
                    nonce: unverified.as_unsigned().nonce.clone(),
                    fee: unverified.as_unsigned().fee.clone(),
                    action: unverified.as_unsigned().action.clone(),
                    network_id: unverified.as_unsigned().network_id,
                    hash: unverified.hash(),
                })
                .collect(),
        }
    }
}
