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

//! Client-side stratum job dispatcher and mining notifier handler

use std::net::{AddrParseError, SocketAddr};
use std::sync::Arc;

use super::super::error::Error as MinerError;
use cstratum::{Error as StratumServiceError, JobDispatcher, PushWorkHandler, Stratum as StratumService};
use primitives::{Bytes, H256, U256};

use super::super::client::Client;
use super::super::miner::work_notify::NotifyWork;
use super::super::miner::{Miner, MinerService};

/// Configures stratum server options.
#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    /// Network address
    pub listen_addr: String,
    /// Port
    pub port: u16,
    /// Secret for peers
    pub secret: Option<H256>,
}

/// Job dispatcher for stratum service
pub struct StratumJobDispatcher {
    client: Arc<Client>,
    miner: Arc<Miner>,
}

impl JobDispatcher for StratumJobDispatcher {
    fn initial(&self) -> Option<String> {
        // initial payload may contain additional data, not in this case
        self.job()
    }

    fn submit(&self, payload: (H256, Vec<Bytes>)) -> Result<(), StratumServiceError> {
        let (pow_hash, seal) = payload;

        ctrace!(STRATUM, "submit_work: Decoded: pow_hash={}, seal={:?}", pow_hash, seal);

        if !self.miner.can_produce_work_package() {
            cwarn!(STRATUM, "Cannot get work package - engine seals internally.");
            return Err(StratumServiceError::InternalError)
        }

        match self.miner.submit_seal(&*self.client, pow_hash, seal) {
            Ok(_) => Ok(()),
            Err(e) => {
                cwarn!(STRATUM, "submit_seal error: {:?}", e);
                Err(StratumServiceError::from(e))
            }
        }
    }
}

impl StratumJobDispatcher {
    /// New stratum job dispatcher given the miner and client
    fn new(miner: Arc<Miner>, client: Arc<Client>) -> StratumJobDispatcher {
        StratumJobDispatcher {
            client,
            miner,
        }
    }

    /// Serializes payload for stratum service
    fn payload(&self, pow_hash: H256, target: U256) -> String {
        format!(r#"["0x{:x}","0x{:x}"]"#, pow_hash, target)
    }
}
/// Wrapper for dedicated stratum service
pub struct Stratum {
    dispatcher: Arc<StratumJobDispatcher>,
    service: Arc<StratumService>,
}

#[derive(Debug)]
/// Stratum error
pub enum Error {
    /// IPC sockets error
    Service(StratumServiceError),
    /// Invalid network address
    Address(AddrParseError),
}

impl From<MinerError> for StratumServiceError {
    fn from(err: MinerError) -> Self {
        match err {
            MinerError::PowHashInvalid => StratumServiceError::PowHashInvalid,
            MinerError::PowInvalid => StratumServiceError::PowInvalid,
            _ => StratumServiceError::InternalError,
        }
    }
}

impl From<StratumServiceError> for Error {
    fn from(service_err: StratumServiceError) -> Error {
        Error::Service(service_err)
    }
}

impl From<AddrParseError> for Error {
    fn from(err: AddrParseError) -> Error {
        Error::Address(err)
    }
}

impl NotifyWork for Stratum {
    fn notify(&self, pow_hash: H256, target: U256) {
        ctrace!(STRATUM, "Notify work");

        self.service
            .push_work_all(self.dispatcher.payload(pow_hash, target))
            .unwrap_or_else(|e| cwarn!(STRATUM, "Error while pushing work: {:?}", e));
    }
}

impl Stratum {
    /// New stratum job dispatcher, given the miner, client and dedicated stratum service
    pub fn start(config: &Config, miner: Arc<Miner>, client: Arc<Client>) -> Result<Stratum, Error> {
        use std::net::IpAddr;

        let dispatcher = Arc::new(StratumJobDispatcher::new(miner, client));
        let stratum_svc = StratumService::start(
            &SocketAddr::new(config.listen_addr.parse::<IpAddr>()?, config.port),
            dispatcher.clone(),
            config.secret.clone(),
        )?;

        Ok(Stratum {
            dispatcher,
            service: stratum_svc,
        })
    }
}
