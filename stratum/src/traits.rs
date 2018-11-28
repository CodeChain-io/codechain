// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std;
use std::error::Error as StdError;

use jsonrpc_core::{Error as JsonError, ErrorCode as JsonErrorCode};
use jsonrpc_tcp_server::PushMessageError;
use primitives::{Bytes, H256};

#[derive(Debug, Clone)]
pub enum Error {
    InternalError,
    PowHashInvalid,
    PowInvalid,
    UnauthorizedWorker,
    NoWork,
    NoWorkers,
    Io(String),
    Tcp(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.description().to_owned())
    }
}

impl From<PushMessageError> for Error {
    fn from(err: PushMessageError) -> Self {
        Error::Tcp(format!("Push message error: {:?}", err))
    }
}

impl From<Error> for JsonError {
    fn from(err: Error) -> Self {
        let (code, message) = match err {
            Error::PowHashInvalid => (21, "Invalid Pow hash".to_string()),
            Error::PowInvalid => (22, "Invalid the nonce".to_string()),
            Error::UnauthorizedWorker => (23, "Unauthorized worker".to_string()),
            _ => (20, "Internal error".to_string()),
        };

        JsonError {
            code: JsonErrorCode::ServerError(code),
            message,
            data: None,
        }
    }
}

/// Interface that can provide pow/blockchain-specific responses for the clients
pub trait JobDispatcher: Send + Sync {
    // json for initial client handshake
    fn initial(&self) -> Option<String> {
        None
    }
    // json for difficulty dispatch
    fn difficulty(&self) -> Option<String> {
        None
    }
    // json for job update given worker_id (payload manager should split job!)
    fn job(&self) -> Option<String> {
        None
    }
    // miner job result
    fn submit(&self, payload: (H256, Vec<Bytes>)) -> Result<(), Error>;
}

/// Interface that can handle requests to push job for workers
pub trait PushWorkHandler: Send + Sync {
    /// push the same work package for all workers (`payload`: json of pow-specific set of work specification)
    fn push_work_all(&self, payload: String) -> Result<(), Error>;

    /// push the work packages worker-wise (`payload`: json of pow-specific set of work specification)
    fn push_work(&self, payloads: Vec<String>) -> Result<(), Error>;
}

pub struct ServiceConfiguration {
    pub listen_addr: String,
    pub port: u16,
    pub secret: Option<H256>,
}
