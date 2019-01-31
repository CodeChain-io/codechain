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

use std::iter::FromIterator;

use cjson::uint::Uint;
use ckey::{NetworkId, PlatformAddress, Public, Signature};
use ctypes::transaction::{Action as ActionType, AssetMintOutput as AssetMintOutputType};
use ctypes::ShardId;
use primitives::{Bytes, H160, H256};
use rustc_serialize::hex::{FromHex, FromHexError, ToHex};

use super::super::errors::ConversionError;
use super::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, OrderOnTransfer};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,

        metadata: String,
        approvals: Vec<Signature>,
        expiration: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,
        inputs: Vec<AssetTransferInput>,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    DecomposeAsset {
        network_id: NetworkId,
        input: Box<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
    },
    Pay {
        receiver: PlatformAddress,
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    #[serde(rename_all = "camelCase")]
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<String>,
        quantity: Uint,
    },
    Store {
        content: String,
        certifier: PlatformAddress,
        signature: Signature,
    },
    Remove {
        hash: H256,
        signature: Signature,
    },
    #[serde(rename_all = "camelCase")]
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ActionWithTracker {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        tracker: H256,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,

        metadata: String,
        approvals: Vec<Signature>,
        expiration: Option<u64>,

        tracker: H256,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        approvals: Vec<Signature>,

        tracker: H256,
    },
    #[serde(rename_all = "camelCase")]
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,
        inputs: Vec<AssetTransferInput>,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        tracker: H256,
    },
    #[serde(rename_all = "camelCase")]
    DecomposeAsset {
        network_id: NetworkId,
        input: Box<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,

        approvals: Vec<Signature>,

        tracker: H256,
    },
    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: Box<AssetTransferInput>,

        tracker: H256,
    },
    Pay {
        receiver: PlatformAddress,
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    #[serde(rename_all = "camelCase")]
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<String>,
        quantity: Uint,
    },
    Store {
        content: String,
        certifier: PlatformAddress,
        signature: Signature,
    },
    Remove {
        hash: H256,
        signature: Signature,
    },
    #[serde(rename_all = "camelCase")]
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

impl ActionWithTracker {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        let tracker = from.tracker();
        match from {
            ActionType::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,

                output,
                approvals,
            } => ActionWithTracker::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                allowed_script_hashes,
                output: Box::new((*output).into()),
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                metadata,
                approvals,
                expiration,
            } => ActionWithTracker::TransferAsset {
                network_id,
                burns: burns.into_iter().map(From::from).collect(),
                inputs: inputs.into_iter().map(From::from).collect(),
                outputs: outputs.into_iter().map(From::from).collect(),
                orders: orders.into_iter().map(From::from).collect(),
                metadata,
                approvals,
                expiration,
                tracker: tracker.unwrap(),
            },
            ActionType::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                approvals,
            } => ActionWithTracker::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                allowed_script_hashes,
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                approvals,
            } => ActionWithTracker::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                allowed_script_hashes,
                inputs: inputs.into_iter().map(From::from).collect(),
                output: Box::new((*output).into()),
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::DecomposeAsset {
                network_id,
                input,
                outputs,
                approvals,
            } => ActionWithTracker::DecomposeAsset {
                network_id,
                input: Box::new(input.into()),
                outputs: outputs.into_iter().map(From::from).collect(),
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::UnwrapCCC {
                network_id,
                burn,
            } => ActionWithTracker::UnwrapCCC {
                network_id,
                burn: Box::new(burn.into()),
                tracker: tracker.unwrap(),
            },
            ActionType::Pay {
                receiver,
                quantity,
            } => ActionWithTracker::Pay {
                receiver: PlatformAddress::new_v1(network_id, receiver),
                quantity: quantity.into(),
            },
            ActionType::SetRegularKey {
                key,
            } => ActionWithTracker::SetRegularKey {
                key,
            },
            ActionType::CreateShard => ActionWithTracker::CreateShard,
            ActionType::SetShardOwners {
                shard_id,
                owners,
            } => ActionWithTracker::SetShardOwners {
                shard_id,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
            },
            ActionType::SetShardUsers {
                shard_id,
                users,
            } => ActionWithTracker::SetShardUsers {
                shard_id,
                users: users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect(),
            },
            ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity,
            } => {
                let parameters = parameters.into_iter().map(|param| param.to_hex()).collect();
                ActionWithTracker::WrapCCC {
                    shard_id,
                    lock_script_hash,
                    parameters,
                    quantity: quantity.into(),
                }
            }
            ActionType::Store {
                content,
                certifier,
                signature,
            } => ActionWithTracker::Store {
                content,
                certifier: PlatformAddress::new_v1(network_id, certifier),
                signature,
            },
            ActionType::Remove {
                hash,
                signature,
            } => ActionWithTracker::Remove {
                hash,
                signature,
            },
            ActionType::Custom {
                handler_id,
                bytes,
            } => ActionWithTracker::Custom {
                handler_id,
                bytes,
            },
        }
    }
}

// FIXME: Use TryFrom.
impl From<Action> for Result<ActionType, ConversionError> {
    fn from(from: Action) -> Self {
        Ok(match from {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                output,
                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                let output_content = Result::<AssetMintOutputType, FromHexError>::from(*output)?;
                ActionType::MintAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    output: Box::new(output_content),
                    approvals,
                }
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,

                metadata,
                approvals,
                expiration,
            } => {
                let iter_outputs = outputs.into_iter().map(From::from);
                let orders = orders.into_iter().map(From::from).collect::<Result<_, _>>()?;
                ActionType::TransferAsset {
                    network_id,
                    burns: burns.into_iter().map(From::from).collect(),
                    inputs: inputs.into_iter().map(From::from).collect(),
                    outputs: Result::from_iter(iter_outputs)?,
                    orders,
                    metadata,
                    approvals,
                    expiration,
                }
            }
            Action::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,

                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                ActionType::ChangeAssetScheme {
                    network_id,
                    shard_id,
                    asset_type,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    approvals,
                }
            }
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,

                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                let output_content = Result::<AssetMintOutputType, FromHexError>::from(*output)?;
                ActionType::ComposeAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    inputs: inputs.into_iter().map(|input| input.into()).collect(),
                    output: Box::new(output_content),
                    approvals,
                }
            }
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,

                approvals,
            } => {
                let iter_outputs = outputs.into_iter().map(From::from);
                ActionType::DecomposeAsset {
                    network_id,
                    input: (*input).into(),
                    outputs: Result::from_iter(iter_outputs)?,
                    approvals,
                }
            }
            Action::UnwrapCCC {
                network_id,
                burn,
            } => ActionType::UnwrapCCC {
                network_id,
                burn: burn.into(),
            },
            Action::Pay {
                receiver,
                quantity,
            } => ActionType::Pay {
                receiver: receiver.try_into_address()?,
                quantity: quantity.into(),
            },
            Action::SetRegularKey {
                key,
            } => ActionType::SetRegularKey {
                key,
            },
            Action::CreateShard => ActionType::CreateShard,
            Action::SetShardOwners {
                shard_id,
                owners,
            } => {
                let owners: Result<_, _> = owners.into_iter().map(PlatformAddress::try_into_address).collect();
                ActionType::SetShardOwners {
                    shard_id,
                    owners: owners?,
                }
            }
            Action::SetShardUsers {
                shard_id,
                users,
            } => {
                let users: Result<_, _> = users.into_iter().map(PlatformAddress::try_into_address).collect();
                ActionType::SetShardUsers {
                    shard_id,
                    users: users?,
                }
            }
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity,
            } => {
                let parameters = parameters.into_iter().map(|param| param.from_hex()).collect::<Result<_, _>>()?;
                ActionType::WrapCCC {
                    shard_id,
                    lock_script_hash,
                    parameters,
                    quantity: quantity.into(),
                }
            }
            Action::Store {
                content,
                certifier,
                signature,
            } => ActionType::Store {
                content,
                certifier: certifier.try_into_address()?,
                signature,
            },
            Action::Remove {
                hash,
                signature,
            } => ActionType::Remove {
                hash,
                signature,
            },
            Action::Custom {
                handler_id,
                bytes,
            } => ActionType::Custom {
                handler_id,
                bytes,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_metadata_with_single_quotations() {
        let mint = ActionWithTracker::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'a single quotation'".to_string(),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: Some(1.into()),
            }
            .into(),

            approvals: vec![],
            tracker: Default::default(),
        };
        let s = to_string(&mint).unwrap();
        let expected = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'a single quotation'","approver":null,"administrator":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[],"tracker":"0x0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert_eq!(&s, expected);
    }

    #[test]
    fn parse_metadata_with_single_quotations() {
        let input = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'a single quotation'","approver":null,"administrator":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[]}"#;
        let mint = from_str(input).unwrap();
        let expected = Action::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'a single quotation'".to_string(),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: Some(1.into()),
            }
            .into(),

            approvals: vec![],
        };
        assert_eq!(expected, mint);
    }

    #[test]
    fn parse_metadata_with_apostrophe() {
        let input = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'an apostrophe’","approver":null,"administrator":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[]}"#;
        let mint = from_str(input).unwrap();
        let expected = Action::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'an apostrophe’".to_string(),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: Some(1.into()),
            }
            .into(),

            approvals: vec![],
        };
        assert_eq!(expected, mint);
    }

    #[test]
    fn serialize_metadata_with_apostrophe() {
        let mint = ActionWithTracker::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'an apostrophe’".to_string(),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: Some(1.into()),
            }
            .into(),

            approvals: vec![],
            tracker: Default::default(),
        };
        let s = to_string(&mint).unwrap();
        let expected = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'an apostrophe’","approver":null,"administrator":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[],"tracker":"0x0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert_eq!(&s, expected);
    }
}
