// Copyright 2019 Kodebox, Inc.
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

use std::sync::Arc;

use client::ConsensusClient;
use ctypes::errors::SyntaxError;
use ctypes::CommonParams;
use ibc::commitment_23::merkle::Proof;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

const ACTION_CREATE_CLIENT: u8 = 1;
const ACTION_UPDATE_CLIENT: u8 = 2;
const ACTION_OPEN_CONNECTION_INIT: u8 = 3;
const ACTION_OPEN_CONNECTION_TRY: u8 = 4;
const ACTION_OPEN_CONNECTION_ACK: u8 = 5;
const ACTION_OPEN_CONNECTION_CONFIRM: u8 = 6;

#[derive(Debug, PartialEq)]
pub enum Action {
    CreateClient {
        id: String,
        kind: u8,
        consensus_state: Vec<u8>,
    },
    UpdateClient {
        id: String,
        header: Vec<u8>,
    },
    OpenConnectionInit {
        id: String,
        client_id: String,
        desired_counterparty_id: String,
        counterparty_client_id: String,
        // NOTE: counterparty_prefix is required according to the ICS spec.
    },
    OpenConnectionTry {
        desired_id: String,
        client_id: String,
        counterparty_connection_id: String,
        counterparty_client_id: String,
        counterparty_versions: Vec<String>,
        proof_init: Proof,
        proof_height: u64,
        consensus_height: u64,
        // NOTE: counterparty_prefix is required according to the ICS spec.
    },
    OpenConnectionAck {
        id: String,
        version: String,
        proof_try: Proof,
        proof_height: u64,
        consensus_height: u64,
    },
    OpenConnectionConfirm {
        id: String,
        proof_ack: Proof,
        proof_height: u64,
    },
}

impl Action {
    pub fn verify(
        &self,
        current_params: &CommonParams,
        client: Option<Arc<dyn ConsensusClient>>,
    ) -> Result<(), SyntaxError> {
        match self {
            Action::CreateClient {
                ..
            } => {}
            Action::UpdateClient {
                ..
            } => {}
            Action::OpenConnectionInit {
                ..
            } => {}
            Action::OpenConnectionTry {
                ..
            } => {}
            Action::OpenConnectionAck {
                ..
            } => {}
            Action::OpenConnectionConfirm {
                ..
            } => {}
        }
        Ok(())
    }
}

impl Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::CreateClient {
                id,
                kind,
                consensus_state,
            } => {
                s.begin_list(4).append(&ACTION_CREATE_CLIENT).append(id).append(kind).append(consensus_state);
            }
            Action::UpdateClient {
                id,
                header,
            } => {
                s.begin_list(3).append(&ACTION_UPDATE_CLIENT).append(id).append(header);
            }
            Action::OpenConnectionInit {
                id,
                client_id,
                desired_counterparty_id,
                counterparty_client_id,
            } => {
                s.begin_list(5)
                    .append(&ACTION_OPEN_CONNECTION_INIT)
                    .append(id)
                    .append(client_id)
                    .append(desired_counterparty_id)
                    .append(counterparty_client_id);
            }
            Action::OpenConnectionTry {
                desired_id,
                client_id,
                counterparty_connection_id,
                counterparty_client_id,
                counterparty_versions,
                proof_init,
                proof_height,
                consensus_height,
            } => {
                s.begin_list(9)
                    .append(&ACTION_OPEN_CONNECTION_TRY)
                    .append(desired_id)
                    .append(client_id)
                    .append(counterparty_connection_id)
                    .append(counterparty_client_id)
                    .append_list::<String, _>(counterparty_versions)
                    .append(proof_init)
                    .append(proof_height)
                    .append(consensus_height);
            }
            Action::OpenConnectionAck {
                id,
                version,
                proof_try,
                proof_height,
                consensus_height,
            } => {
                s.begin_list(6)
                    .append(&ACTION_OPEN_CONNECTION_ACK)
                    .append(id)
                    .append(version)
                    .append(proof_try)
                    .append(proof_height)
                    .append(consensus_height);
            }
            Action::OpenConnectionConfirm {
                id,
                proof_ack,
                proof_height,
            } => {
                s.begin_list(4)
                    .append(&ACTION_OPEN_CONNECTION_CONFIRM)
                    .append(id)
                    .append(proof_ack)
                    .append(proof_height);
            }
        };
    }
}

impl Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        match tag {
            ACTION_CREATE_CLIENT => {
                let item_count = rlp.item_count()?;
                if item_count != 4 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 4,
                        got: item_count,
                    })
                }
                Ok(Action::CreateClient {
                    id: rlp.val_at(1)?,
                    kind: rlp.val_at(2)?,
                    consensus_state: rlp.val_at(3)?,
                })
            }
            ACTION_UPDATE_CLIENT => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(Action::UpdateClient {
                    id: rlp.val_at(1)?,
                    header: rlp.val_at(2)?,
                })
            }
            ACTION_OPEN_CONNECTION_INIT => {
                let item_count = rlp.item_count()?;
                let expected = 5;
                if item_count != expected {
                    return Err(DecoderError::RlpInvalidLength {
                        expected,
                        got: item_count,
                    })
                }
                Ok(Action::OpenConnectionInit {
                    id: rlp.val_at(1)?,
                    client_id: rlp.val_at(2)?,
                    desired_counterparty_id: rlp.val_at(3)?,
                    counterparty_client_id: rlp.val_at(4)?,
                })
            }
            ACTION_OPEN_CONNECTION_TRY => {
                let item_count = rlp.item_count()?;
                let expected = 9;
                if item_count != expected {
                    return Err(DecoderError::RlpInvalidLength {
                        expected,
                        got: item_count,
                    })
                }
                Ok(Action::OpenConnectionTry {
                    desired_id: rlp.val_at(1)?,
                    client_id: rlp.val_at(2)?,
                    counterparty_connection_id: rlp.val_at(3)?,
                    counterparty_client_id: rlp.val_at(4)?,
                    counterparty_versions: rlp.list_at(5)?,
                    proof_init: rlp.val_at(6)?,
                    proof_height: rlp.val_at(7)?,
                    consensus_height: rlp.val_at(8)?,
                })
            }
            ACTION_OPEN_CONNECTION_ACK => {
                let item_count = rlp.item_count()?;
                let expected = 6;
                if item_count != expected {
                    return Err(DecoderError::RlpInvalidLength {
                        expected,
                        got: item_count,
                    })
                }
                Ok(Action::OpenConnectionAck {
                    id: rlp.val_at(1)?,
                    version: rlp.val_at(2)?,
                    proof_try: rlp.val_at(3)?,
                    proof_height: rlp.val_at(4)?,
                    consensus_height: rlp.val_at(5)?,
                })
            }
            ACTION_OPEN_CONNECTION_CONFIRM => {
                let item_count = rlp.item_count()?;
                let expected = 4;
                if item_count != expected {
                    return Err(DecoderError::RlpInvalidLength {
                        expected,
                        got: item_count,
                    })
                }
                Ok(Action::OpenConnectionConfirm {
                    id: rlp.val_at(1)?,
                    proof_ack: rlp.val_at(2)?,
                    proof_height: rlp.val_at(3)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected IBC Action Type")),
        }
    }
}
