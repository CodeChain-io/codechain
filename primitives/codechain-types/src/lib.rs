extern crate ethcore_bytes as ebytes;
extern crate ethereum_types;

pub use ebytes::Bytes;
pub use ethereum_types::{H1024, H128, H160, H256, H264, H32, H512, H520, H64};
pub use ethereum_types::{U128, U256, U512};

pub type Address = H160;
pub type Secret = H256;
pub type Public = H512;

pub mod bytes {
    pub use ebytes::ToPretty;
}
