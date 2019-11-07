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

mod actions;

use self::actions::Action;
use crate::client::ConsensusClient;
use ckey::{Address, Public};
use cstate::{ActionHandler, StateResult, TopLevelState};
use ctypes::errors::RuntimeError;
use ctypes::errors::SyntaxError;
use ctypes::{CommonParams, Header};
use ibc::client_02 as ibc_client;
use ibc::client_02::codechain as ibc_codechain;
use ibc::commitment_23::merkle::Proof;
use ibc::connection_03 as ibc_connection;
use ibc::context as ibc_context;
use parking_lot::RwLock;
use rlp::{Decodable, UntrustedRlp};
use std::sync::{Arc, Weak};

pub const CUSTOM_ACTION_HANDLER_ID: u64 = 3;

pub struct IBC {
    client: RwLock<Option<Weak<dyn ConsensusClient>>>,
}

impl IBC {
    pub fn new() -> Self {
        IBC {
            client: Default::default(),
        }
    }

    pub fn register_resources(&self, client: Weak<dyn ConsensusClient>) {
        *self.client.write() = Some(Weak::clone(&client));
    }
}

impl ActionHandler for IBC {
    fn name(&self) -> &'static str {
        "IBC handler"
    }

    fn handler_id(&self) -> u64 {
        CUSTOM_ACTION_HANDLER_ID
    }

    fn init(&self, state: &mut TopLevelState) -> StateResult<()> {
        Ok(())
    }

    fn execute(
        &self,
        bytes: &[u8],
        state: &mut TopLevelState,
        fee_payer: &Address,
        sender_public: &Public,
    ) -> StateResult<()> {
        let action = Action::decode(&UntrustedRlp::new(bytes)).expect("Verification passed");
        match action {
            Action::CreateClient {
                id,
                kind,
                consensus_state,
            } => create_client(state, fee_payer, &id, kind, &consensus_state),
            Action::UpdateClient {
                id,
                header,
            } => update_client(state, &id, &header),
            Action::OpenConnectionInit {
                id,
                client_id,
                desired_counterparty_id,
                counterparty_client_id,
            } => open_connection_init(state, &id, &client_id, &desired_counterparty_id, &counterparty_client_id),
            Action::OpenConnectionTry {
                desired_id,
                client_id,
                counterparty_connection_id,
                counterparty_client_id,
                counterparty_versions,
                proof_init,
                proof_height,
                consensus_height,
            } => open_connection_try(
                state,
                &desired_id,
                &client_id,
                &counterparty_connection_id,
                &counterparty_client_id,
                &counterparty_versions,
                &proof_init,
                &proof_height,
                &consensus_height,
            ),
            Action::OpenConnectionAck {
                id,
                version,
                proof_try,
                proof_height,
                consensus_height,
            } => open_connection_ack(state, &id, &version, &proof_try, &proof_height, &consensus_height),
            Action::OpenConnectionConfirm {
                id,
                proof_ack,
                proof_height,
            } => open_connection_confirm(state, &id, &proof_ack, &proof_height),
        }
    }

    fn verify(&self, bytes: &[u8], current_params: &CommonParams) -> Result<(), SyntaxError> {
        let action = Action::decode(&UntrustedRlp::new(bytes))
            .map_err(|err| SyntaxError::InvalidCustomAction(err.to_string()))?;
        let client: Option<Arc<dyn ConsensusClient>> = self.client.read().as_ref().and_then(Weak::upgrade);
        action.verify(current_params, client)
    }

    fn on_close_block(
        &self,
        _state: &mut TopLevelState,
        _header: &Header,
        _parent_header: &Header,
        _parent_common_params: &CommonParams,
    ) -> StateResult<()> {
        Ok(())
    }
}

fn create_client(
    state: &mut TopLevelState,
    fee_payer: &Address,
    id: &str,
    kind: ibc_client::Kind,
    consensus_state: &[u8],
) -> StateResult<()> {
    let mut context = ibc_context::TopLevelContext::new(state);
    let client_manager = ibc_client::Manager::new();
    if kind != ibc_client::KIND_CODECHAIN {
        return Err(RuntimeError::IBC(format!("CreateClient has invalid type {}", kind)).into())
    }
    let rlp = rlp::UntrustedRlp::new(consensus_state);
    let codechain_consensus_state: ibc_codechain::ConsensusState = match rlp.as_val() {
        Ok(cs) => cs,
        Err(err) => {
            return Err(RuntimeError::IBC(format!("CreateClient failed to decode consensus state {}", err)).into())
        }
    };

    client_manager
        .create(&mut context, id, &codechain_consensus_state)
        .map_err(|err| RuntimeError::IBC(format!("CreateClient: {:?}", err)))?;
    Ok(())
}

fn update_client(state: &mut TopLevelState, id: &str, header: &[u8]) -> StateResult<()> {
    let mut context = ibc_context::TopLevelContext::new(state);
    let client_manager = ibc_client::Manager::new();
    let client_state = client_manager.query(&mut context, id).map_err(RuntimeError::IBC)?;

    client_state.update(&mut context, header).map_err(RuntimeError::IBC)?;

    Ok(())
}

fn open_connection_init(
    state: &mut TopLevelState,
    id: &str,
    client_id: &str,
    desired_counterparty_id: &str,
    counterparty_client_id: &str,
) -> StateResult<()> {
    let mut context = ibc_context::TopLevelContext::new(state);
    let client_manager = ibc_client::Manager::new();
    let connection_manager = ibc_connection::Manager::new(client_manager);

    connection_manager
        .create(&mut context, id, client_id, desired_counterparty_id, counterparty_client_id)
        .map_err(|err| RuntimeError::IBC(format!("OpenConnectionInit: {:?}", err)))?;

    Ok(())

    // 1. create client_manager and check client_id exists.
    // 2. create connection_manager and check connection_id exists.
    // 3. save ConnectionEnd
}

fn open_connection_try(
    state: &mut TopLevelState,
    desired_id: &str,
    client_id: &str,
    counterparty_connection_id: &str,
    counterparty_client_id: &str,
    counterparty_versions: &Vec<String>,
    proof_init: &Proof,
    proof_height: &u64,
    consensus_height: &u64,
) -> StateResult<()> {
    unimplemented!()
    // 1. create
    // check desired id
    // pick version
}

fn open_connection_ack(
    state: &mut TopLevelState,
    id: &str,
    version: &str,
    proof_try: &Proof,
    proof_height: &u64,
    consensus_height: &u64,
) -> StateResult<()> {
    unimplemented!()
}

fn open_connection_confirm(
    state: &mut TopLevelState,
    id: &str,
    proof_ack: &Proof,
    proof_height: &u64,
) -> StateResult<()> {
    unimplemented!()
}
