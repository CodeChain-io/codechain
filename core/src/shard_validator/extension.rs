// Copyright 2018 Kodebox, Inc.
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

use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;

use ckey::{Address, Signature};
use cnetwork::{Api, NetworkExtension, NodeId};
use ctypes::parcel::Action;
use parking_lot::RwLock;
use primitives::H256;
use rlp::{Decodable, Encodable, UntrustedRlp};

use super::super::{AccountProvider, AccountProviderError, Client};
use super::client::ShardValidatorClient;
use super::message::Message;

pub struct ShardValidator {
    #[allow(dead_code)]
    client: Arc<Client>,

    account: Option<Address>,
    account_provider: Arc<AccountProvider>,

    api: RwLock<Option<Arc<Api>>>,
    nodes: RwLock<HashSet<NodeId>>,

    actions: RwLock<HashMap<H256, RegisteredAction>>,
    signatures: RwLock<HashMap<H256, HashSet<Signature>>>,
}

enum RegisterActionOutcome {
    Signed(Signature),
    Registered,
    AlreadyExists,
}

impl ShardValidator {
    pub fn new(client: Arc<Client>, account: Option<Address>, account_provider: Arc<AccountProvider>) -> Arc<Self> {
        Arc::new(Self {
            client,

            account,
            account_provider,
            api: RwLock::default(),
            nodes: RwLock::default(),
            actions: RwLock::default(),
            signatures: RwLock::default(),
        })
    }
}

fn register_action(
    action: RegisteredAction,
    actions: &mut HashMap<H256, RegisteredAction>,
    account_provider: &AccountProvider,
    account: &Option<Address>,
) -> Result<RegisterActionOutcome, AccountProviderError> {
    let action_hash = action.hash();

    let t = actions.insert(action_hash, action);

    if t.is_none() {
        if let Some(account) = account.as_ref() {
            account_provider.sign(*account, None, action_hash).map(RegisterActionOutcome::Signed)
        } else {
            Ok(RegisterActionOutcome::Registered)
        }
    } else {
        Ok(RegisterActionOutcome::AlreadyExists)
    }
}

impl ShardValidatorClient for ShardValidator {
    fn register_action(&self, action: Action) -> bool {
        let mut actions = self.actions.write();
        match register_action(RegisteredAction::Local(action), &mut actions, &self.account_provider, &self.account) {
            Err(_) => false,
            Ok(RegisterActionOutcome::AlreadyExists) => false,
            _ => true,
        }
    }

    fn signatures(&self, action_hash: &H256) -> Vec<Signature> {
        let signatures = self.signatures.read();
        match signatures.get(&action_hash) {
            Some(signatures) => signatures.iter().map(Clone::clone).collect(),
            None => vec![],
        }
    }
}

impl NetworkExtension for ShardValidator {
    fn name(&self) -> &'static str {
        "shard-validator"
    }
    fn need_encryption(&self) -> bool {
        false
    }
    fn versions(&self) -> &[u64] {
        const VERSIONS: &'static [u64] = &[0];
        &VERSIONS
    }

    fn on_initialize(&self, api: Arc<Api>) {
        let mut api_lock = self.api.write();
        *api_lock = Some(api);
    }

    fn on_node_added(&self, node: &NodeId, _version: u64) {
        let mut nodes = self.nodes.write();
        let t = nodes.insert(*node);
        debug_assert!(t);
    }

    fn on_node_removed(&self, node: &NodeId) {
        let mut nodes = self.nodes.write();
        nodes.remove(node);
    }

    fn on_message(&self, from: &NodeId, message: &[u8]) {
        let message = match Message::decode(&UntrustedRlp::new(&message)) {
            Ok(message) => message,
            Err(err) => {
                cwarn!(SHARD_VALIDATOR, "Invalid message from {:?}: {:?}", from, err);
                return
            }
        };
        match message {
            Message::Action(action) => {
                let api = self.api.read();
                let nodes = self.nodes.read();
                let mut actions = self.actions.write();
                let mut signatures_map = self.signatures.write();

                let action_hash = action.hash();

                match register_action(
                    RegisteredAction::External(action.clone()),
                    &mut actions,
                    &self.account_provider,
                    &self.account,
                ) {
                    Err(err) => {
                        cerror!(SHARD_VALIDATOR, "Cannot sign new action {:?}", err);
                    }
                    Ok(RegisterActionOutcome::AlreadyExists) => return,
                    Ok(RegisterActionOutcome::Registered) => {}
                    Ok(RegisterActionOutcome::Signed(signature)) => {
                        let signatures = vec![signature];
                        let new_signatures = insert_signatures(&mut signatures_map, action_hash, &signatures);
                        debug_assert_eq!(1, new_signatures.len());

                        let message = Message::Signatures {
                            action_hash,
                            signatures,
                        }.rlp_bytes();

                        let api = api.as_ref().expect("The extension must be initialized first");
                        for node in nodes.iter() {
                            api.send(node, &message);
                        }
                    }
                }

                cinfo!(SHARD_VALIDATOR, "New action({:?}) is received from {:?}", action, from);
                let api = api.as_ref().expect("The extension must be initialized first");

                let message = Message::Action(action).rlp_bytes();
                for node in nodes.iter().filter(|node| node != &from) {
                    api.send(node, &message);
                }
            }
            Message::Signatures {
                action_hash,
                signatures,
            } => {
                let api = self.api.read();
                let nodes = self.nodes.read();
                let mut signatures_map = self.signatures.write();

                let new_signatures = insert_signatures(&mut signatures_map, action_hash, &signatures);
                if !new_signatures.is_empty() {
                    cinfo!(SHARD_VALIDATOR, "New signatures({:?}) is received from {:?}", new_signatures, from);
                    let message = Message::Signatures {
                        action_hash,
                        signatures: new_signatures,
                    }.rlp_bytes();
                    let api = api.as_ref().expect("The extension must be initialized first");
                    for node in nodes.iter().filter(|node| node != &from) {
                        api.send(node, &message);
                    }
                }
            }
            Message::RequestAction(action_hash) => {
                let api = self.api.read();
                let actions = self.actions.read();

                if let Some(action) = actions.get(&action_hash) {
                    let api = api.as_ref().expect("The extension must be initialized first");
                    api.send(from, &Message::Action((*action).clone()).rlp_bytes());
                }
            }
        }
    }
}


fn insert_signatures(
    signatures_map: &mut HashMap<H256, HashSet<Signature>>,
    action_hash: H256,
    signatures: &[Signature],
) -> Vec<Signature> {
    let signatures_set = signatures_map.entry(action_hash).or_insert_with(HashSet::new);
    let mut new_signatures = Vec::with_capacity(signatures.len());
    for signature in signatures.into_iter() {
        let t = signatures_set.insert(*signature);
        if t {
            new_signatures.push(signature.clone());
        }
    }
    new_signatures
}

enum RegisteredAction {
    Local(Action),
    External(Action),
}

impl Deref for RegisteredAction {
    type Target = Action;

    fn deref(&self) -> &<Self as Deref>::Target {
        match self {
            RegisteredAction::Local(action) => action,
            RegisteredAction::External(action) => action,
        }
    }
}
