// Copyright 2020 Kodebox, Inc.
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

use crate::config::load_config;
use ccore::stake::Action::SelfNominate;
use ccore::stake::{Banned, Candidates, Jail, CUSTOM_ACTION_HANDLER_ID};
use ccore::{
    AccountProvider, AccountProviderError, BlockId, ConsensusClient, SignedTransaction, UnverifiedTransaction,
};
use ckey::PlatformAddress;
use ckey::{Address, Public, Signature};
use ckeystore::DecryptedAccount;
use clap::ArgMatches;
use codechain_types::transaction::{Action, Transaction};
use primitives::{Bytes, H256};
use rlp::Encodable;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const NEED_NOMINATION_UNDER_TERM_LEFT: u64 = 3;
#[derive(Clone)]
struct SelfSigner {
    account_provider: Arc<AccountProvider>,
    signer: Option<(Address, Public)>,
    decrypted_account: Option<DecryptedAccount>,
}
impl SelfSigner {
    pub fn new(ap: Arc<AccountProvider>, address: Address) -> Self {
        let public = {
            let account = ap.get_unlocked_account(&address).expect("The address must be registered in AccountProvider");
            account.public().expect("Cannot get public from account")
        };
        Self {
            account_provider: ap,
            signer: Some((address, public)),
            decrypted_account: None,
        }
    }

    pub fn sign_ecdsa(&self, hash: H256) -> Result<Signature, AccountProviderError> {
        let address = self.signer.map(|(address, _public)| address).unwrap_or_else(Default::default);
        let result = match &self.decrypted_account {
            Some(account) => account.sign(&hash)?,
            None => {
                let account = self.account_provider.get_unlocked_account(&address)?;
                account.sign(&hash)?
            }
        };
        Ok(result)
    }

    pub fn address(&self) -> Option<&Address> {
        self.signer.as_ref().map(|(address, _)| address)
    }
}

pub struct AutoSelfNomination {
    client: Arc<dyn ConsensusClient>,
    signer: SelfSigner,
}

impl AutoSelfNomination {
    pub fn new(client: Arc<dyn ConsensusClient>, ap: Arc<AccountProvider>, address: Address) -> Arc<Self> {
        Arc::new(Self {
            client,
            signer: SelfSigner::new(ap, address),
        })
    }

    pub fn send_self_nominate_transaction(&self, matches: &ArgMatches) {
        let config = load_config(matches).unwrap();
        let account_address = config.mining.engine_signer.unwrap();
        let defualt_metadata = config.mining.self_nomination_metadata.unwrap();
        let target_deposite = config.mining.self_target_deposit.unwrap();
        let interval = config.mining.self_nomination_interval.unwrap();
        let self_client = self.client.clone();
        let self_signer = self.signer.clone();
        thread::Builder::new()
            .name("Auto Self Nomination".to_string())
            .spawn(move || loop {
                AutoSelfNomination::send(
                    &self_client,
                    &self_signer,
                    &account_address,
                    &defualt_metadata,
                    target_deposite,
                );
                thread::sleep(Duration::from_millis(interval));
            })
            .unwrap();
    }

    fn send(
        client: &Arc<dyn ConsensusClient>,
        signer: &SelfSigner,
        account_address: &PlatformAddress,
        metadata: &str,
        targetdep: u64,
    ) {
        let metabytes = metadata.rlp_bytes();
        let mut dep = targetdep;
        let address = account_address.address();
        let block_id = BlockId::Latest;
        let state = client.state_at(block_id).unwrap();
        let current_term = client.current_term_id(block_id).unwrap();
        let banned = Banned::load_from_state(&state).unwrap();
        if banned.is_banned(address) {
            cwarn!(ENGINE, "Account is banned");
            return
        }
        let jailed = Jail::load_from_state(&state).unwrap();
        if jailed.get_prisoner(&address).is_some() {
            let prisoner = jailed.get_prisoner(&address).unwrap();

            if prisoner.custody_until <= (current_term) {
                cwarn!(ENGINE, "Account is still in custody");
                return
            }
        }
        let candidate = Candidates::load_from_state(&state).unwrap();
        if candidate.get_candidate(&address).is_some() {
            let candidate_need_nomination = candidate.get_candidate(&address).unwrap();
            if candidate_need_nomination.nomination_ends_at + NEED_NOMINATION_UNDER_TERM_LEFT <= current_term {
                cdebug!(
                    ENGINE,
                    "No need self nominate. nomination_ends_at: {}, current_term: {}",
                    candidate_need_nomination.nomination_ends_at,
                    current_term
                );
                return
            }
            if candidate_need_nomination.deposit.lt(&targetdep) {
                dep = targetdep.min(targetdep);
            } else {
                dep = 0 as u64;
            }
        }

        AutoSelfNomination::self_nomination_transaction(&client, &signer, dep, metabytes);
    }

    fn self_nomination_transaction(
        client: &Arc<dyn ConsensusClient>,
        signer: &SelfSigner,
        deposit: u64,
        metadata: Bytes,
    ) {
        let network_id = client.network_id();
        let seq = match signer.address() {
            Some(address) => client.latest_seq(address),
            None => {
                cwarn!(ENGINE, "Signer was not assigned");
                return
            }
        };
        let selfnominate = SelfNominate {
            deposit,
            metadata,
        };
        let tx = Transaction {
            seq,
            fee: 0,
            network_id,
            action: Action::Custom {
                handler_id: CUSTOM_ACTION_HANDLER_ID,
                bytes: selfnominate.rlp_bytes(),
            },
        };

        let signature = match signer.sign_ecdsa(*tx.hash()) {
            Ok(signature) => signature,
            Err(e) => {
                cerror!(ENGINE, "Could not sign the message:{}", e);
                return
            }
        };
        let unverified = UnverifiedTransaction::new(tx, signature);
        let signed = SignedTransaction::try_new(unverified).expect("secret is valid so it's recoverable");

        match client.queue_own_transaction(signed) {
            Ok(_) => {
                cinfo!(ENGINE, "Send self nominate transaction");
            }
            Err(e) => {
                cerror!(ENGINE, "Failed to queue self nominate transaction: {}", e);
            }
        }
    }
}
