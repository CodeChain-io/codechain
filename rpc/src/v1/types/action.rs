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

use std::convert::TryFrom;

use cjson::uint::Uint;
use ckey::{NetworkId, PlatformAddress, Public, Signature};
use ctypes::transaction::{Action as ActionType, AssetMintOutput as AssetMintOutputType};
use ctypes::{ShardId, Tracker};
use primitives::{Bytes, H160, H256};
use rustc_serialize::hex::{FromHex, ToHex};

use super::super::errors::ConversionError;
use super::{AssetMintOutput, AssetTransferInput, AssetTransferOutput};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        registrar: Option<PlatformAddress>,
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

        metadata: String,
        approvals: Vec<Signature>,
        expiration: Option<Uint>,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: u64,
        metadata: String,
        approver: Option<PlatformAddress>,
        registrar: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    IncreaseAssetSupply {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: u64,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
        receiver: PlatformAddress,
    },
    Pay {
        receiver: PlatformAddress,
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard {
        users: Vec<PlatformAddress>,
    },
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
        payer: PlatformAddress,
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
        handler_id: Uint,
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
        registrar: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        tracker: Tracker,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        // NOTE: The orders field is removed in the core but it remains to
        // support the old version of the SDK
        orders: Vec<()>,

        metadata: String,
        approvals: Vec<Signature>,
        expiration: Option<Uint>,

        tracker: Tracker,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: u64,
        metadata: String,
        approver: Option<PlatformAddress>,
        registrar: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        approvals: Vec<Signature>,

        tracker: Tracker,
    },
    #[serde(rename_all = "camelCase")]
    IncreaseAssetSupply {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: u64,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        tracker: Tracker,
    },

    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: Box<AssetTransferInput>,
        receiver: PlatformAddress,

        tracker: Tracker,
    },
    Pay {
        receiver: PlatformAddress,
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard {
        users: Vec<PlatformAddress>,
    },
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
        payer: PlatformAddress,
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
        handler_id: Uint,
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
                registrar,
                allowed_script_hashes,

                output,
                approvals,
            } => ActionWithTracker::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                registrar: registrar.map(|registrar| PlatformAddress::new_v1(network_id, registrar)),
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
                metadata,
                approvals,
                expiration,
            } => ActionWithTracker::TransferAsset {
                network_id,
                burns: burns.into_iter().map(From::from).collect(),
                inputs: inputs.into_iter().map(From::from).collect(),
                outputs: outputs.into_iter().map(From::from).collect(),
                orders: vec![],
                metadata,
                approvals,
                expiration: expiration.map(From::from),
                tracker: tracker.unwrap(),
            },
            ActionType::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                approvals,
            } => ActionWithTracker::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq: seq as u64,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                registrar: registrar.map(|registrar| PlatformAddress::new_v1(network_id, registrar)),
                allowed_script_hashes,
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq,
                output,
                approvals,
            } => ActionWithTracker::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq: seq as u64,
                output: Box::new((*output).into()),
                approvals,
                tracker: tracker.unwrap(),
            },
            ActionType::UnwrapCCC {
                network_id,
                burn,
                receiver,
            } => ActionWithTracker::UnwrapCCC {
                network_id,
                burn: Box::new(burn.into()),
                receiver: PlatformAddress::new_v1(network_id, receiver),
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
            ActionType::CreateShard {
                users,
            } => {
                let users = users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect();
                ActionWithTracker::CreateShard {
                    users,
                }
            }
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
                payer,
            } => {
                let parameters = parameters.into_iter().map(|param| param.to_hex()).collect();
                let payer = PlatformAddress::new_v1(network_id, payer);
                ActionWithTracker::WrapCCC {
                    shard_id,
                    lock_script_hash,
                    parameters,
                    quantity: quantity.into(),
                    payer,
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
                handler_id: handler_id.into(),
                bytes,
            },
        }
    }
}

impl TryFrom<Action> for ActionType {
    type Error = ConversionError;
    fn try_from(from: Action) -> Result<Self, Self::Error> {
        Ok(match from {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                output,
                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let registrar = match registrar {
                    Some(registrar) => Some(registrar.try_into_address()?),
                    None => None,
                };
                let output_content = AssetMintOutputType::try_from(*output)?;
                ActionType::MintAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    registrar,
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

                metadata,
                approvals,
                expiration,
            } => {
                let outputs = outputs.into_iter().map(TryFrom::try_from).collect::<Result<_, _>>()?;
                ActionType::TransferAsset {
                    network_id,
                    burns: burns.into_iter().map(From::from).collect(),
                    inputs: inputs.into_iter().map(From::from).collect(),
                    outputs,
                    metadata,
                    approvals,
                    expiration: expiration.map(From::from),
                }
            }
            Action::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,

                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let registrar = match registrar {
                    Some(registrar) => Some(registrar.try_into_address()?),
                    None => None,
                };
                ActionType::ChangeAssetScheme {
                    network_id,
                    shard_id,
                    asset_type,
                    seq: seq as usize,
                    metadata,
                    approver,
                    registrar,
                    allowed_script_hashes,
                    approvals,
                }
            }
            Action::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq,
                output,
                approvals,
            } => {
                let output_content = AssetMintOutputType::try_from(*output)?;
                ActionType::IncreaseAssetSupply {
                    network_id,
                    shard_id,
                    seq: seq as usize,
                    asset_type,
                    output: Box::new(output_content),
                    approvals,
                }
            }
            Action::UnwrapCCC {
                network_id,
                burn,
                receiver,
            } => ActionType::UnwrapCCC {
                network_id,
                burn: burn.into(),
                receiver: receiver.try_into_address()?,
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
            Action::CreateShard {
                users,
            } => {
                let users = users.into_iter().map(PlatformAddress::try_into_address).collect::<Result<_, _>>()?;
                ActionType::CreateShard {
                    users,
                }
            }
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
                payer,
            } => {
                let parameters = parameters.into_iter().map(|param| param.from_hex()).collect::<Result<_, _>>()?;
                ActionType::WrapCCC {
                    shard_id,
                    lock_script_hash,
                    parameters,
                    quantity: quantity.into(),
                    payer: payer.try_into_address()?,
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
                handler_id: handler_id.into(),
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
            registrar: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: 1.into(),
            }
            .into(),

            approvals: vec![],
            tracker: Default::default(),
        };
        let s = to_string(&mint).unwrap();
        let expected = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'a single quotation'","approver":null,"registrar":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[],"tracker":"0x0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert_eq!(&s, expected);
    }

    #[test]
    fn parse_metadata_with_single_quotations() {
        let input = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'a single quotation'","approver":null,"registrar":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[]}"#;
        let mint = from_str(input).unwrap();
        let expected = Action::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'a single quotation'".to_string(),
            approver: None,
            registrar: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: 1.into(),
            }
            .into(),

            approvals: vec![],
        };
        assert_eq!(expected, mint);
    }

    #[test]
    fn parse_metadata_with_apostrophe() {
        let input = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'an apostrophe’","approver":null,"registrar":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[]}"#;
        let mint = from_str(input).unwrap();
        let expected = Action::MintAsset {
            network_id: "ab".into(),
            shard_id: 0,
            metadata: "string with 'an apostrophe’".to_string(),
            approver: None,
            registrar: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: 1.into(),
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
            registrar: None,
            allowed_script_hashes: vec![],

            output: AssetMintOutput {
                lock_script_hash: Default::default(),
                parameters: vec![],
                supply: 1.into(),
            }
            .into(),

            approvals: vec![],
            tracker: Default::default(),
        };
        let s = to_string(&mint).unwrap();
        let expected = r#"{"type":"mintAsset","networkId":"ab","shardId":0,"metadata":"string with 'an apostrophe’","approver":null,"registrar":null,"allowedScriptHashes":[],"output":{"lockScriptHash":"0x0000000000000000000000000000000000000000","parameters":[],"supply":"0x1"},"approvals":[],"tracker":"0x0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert_eq!(&s, expected);
    }
}
