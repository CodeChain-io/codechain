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
    let context = ibc_context::TopLevelContext::new(state);
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
        .create(&context, id, &codechain_consensus_state)
        .map_err(|err| RuntimeError::IBC(format!("CreateClient: {:?}", err)))?;
    Ok(())
}
