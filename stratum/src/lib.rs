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

//! Stratum protocol implementation for CodeChain clients

extern crate codechain_crypto as ccrypto;
#[macro_use]
extern crate codechain_logger as clogger;
extern crate codechain_json as cjson;
extern crate jsonrpc_core;
extern crate jsonrpc_macros;
extern crate jsonrpc_tcp_server;
extern crate parking_lot;
extern crate primitives;

#[macro_use]
extern crate log;

#[cfg(test)]
extern crate tokio_core;
#[cfg(test)]
extern crate tokio_io;

mod traits;

pub use crate::traits::{Error, JobDispatcher, PushWorkHandler, ServiceConfiguration};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use ccrypto::blake256;
use cjson::bytes::Bytes;
use jsonrpc_core::{to_value, Compatibility, MetaIoHandler, Metadata, Params};
use jsonrpc_macros::IoDelegate;
use jsonrpc_tcp_server::{
    Dispatcher, MetaExtractor, PushMessageError, RequestContext, Server as JsonRpcServer,
    ServerBuilder as JsonRpcServerBuilder,
};
use parking_lot::RwLock;
use primitives::H256;

type RpcResult = Result<jsonrpc_core::Value, jsonrpc_core::Error>;

const NOTIFY_COUNTER_INITIAL: u32 = 16;

/// Container which owns rpc server and stratum implementation
pub struct Stratum {
    /// RPC server
    ///
    /// It is an `Option` so it can be easily closed and released during `drop` phase
    rpc_server: Option<JsonRpcServer>,
    /// stratum protocol implementation
    ///
    /// It is owned by a container and rpc server
    implementation: Arc<StratumImpl>,
    /// Message dispatcher (tcp/ip service)
    ///
    /// Used to push messages to peers
    tcp_dispatcher: Dispatcher,
}

impl Stratum {
    pub fn start(
        addr: &SocketAddr,
        dispatcher: Arc<JobDispatcher>,
        secret: Option<H256>,
    ) -> Result<Arc<Stratum>, Error> {
        let implementation = Arc::new(StratumImpl {
            subscribers: RwLock::new(Vec::new()),
            job_que: RwLock::new(HashSet::new()),
            dispatcher,
            workers: Arc::new(RwLock::new(HashMap::new())),
            secret,
            notify_counter: RwLock::new(NOTIFY_COUNTER_INITIAL),
        });

        let mut delegate = IoDelegate::<StratumImpl, SocketMetadata>::new(implementation.clone());
        delegate.add_method_with_meta("mining.subscribe", StratumImpl::subscribe);
        delegate.add_method_with_meta("mining.authorize", StratumImpl::authorize);
        delegate.add_method_with_meta("mining.submit", StratumImpl::submit);
        let mut handler = MetaIoHandler::<SocketMetadata>::with_compatibility(Compatibility::Both);
        handler.extend_with(delegate);

        let server_builder = JsonRpcServerBuilder::new(handler);
        let tcp_dispatcher = server_builder.dispatcher();
        let server_builder = server_builder.session_meta_extractor(PeerMetaExtractor::new(tcp_dispatcher.clone()));
        let server = server_builder.start(addr)?;

        let stratum = Arc::new(Stratum {
            rpc_server: Some(server),
            implementation,
            tcp_dispatcher,
        });

        Ok(stratum)
    }
}

impl PushWorkHandler for Stratum {
    fn push_work_all(&self, payload: String) -> Result<(), Error> {
        self.implementation.push_work_all(payload.as_str(), &self.tcp_dispatcher)
    }

    fn push_work(&self, payloads: Vec<String>) -> Result<(), Error> {
        self.implementation.push_work(payloads, &self.tcp_dispatcher)
    }
}

impl Drop for Stratum {
    fn drop(&mut self) {
        // shut down rpc server
        self.rpc_server.take().map(|server| server.close());
    }
}

struct StratumImpl {
    /// Subscribed clients
    subscribers: RwLock<Vec<SocketAddr>>,
    /// List of workers supposed to receive job update
    job_que: RwLock<HashSet<SocketAddr>>,
    /// Payload manager
    dispatcher: Arc<JobDispatcher>,
    /// Authorized workers (socket - worker_id)
    workers: Arc<RwLock<HashMap<SocketAddr, String>>>,
    /// Secret if any
    secret: Option<H256>,
    /// Dispatch notify counter
    notify_counter: RwLock<u32>,
}

trait StratumRpc {
    fn subscribe(&self, _params: Params, meta: SocketMetadata) -> RpcResult;
    fn authorize(&self, params: Params, meta: SocketMetadata) -> RpcResult;
    fn submit(&self, params: Params, meta: SocketMetadata) -> RpcResult;
}

impl StratumRpc for StratumImpl {
    /// rpc method `mining.subscribe`
    fn subscribe(&self, _params: Params, meta: SocketMetadata) -> RpcResult {
        use std::str::FromStr;

        self.subscribers.write().push(*meta.addr());
        self.job_que.write().insert(*meta.addr());
        ctrace!(STRATUM, "Subscription request from {:?}", meta.addr());

        Ok(match self.dispatcher.initial() {
            Some(initial) => match jsonrpc_core::Value::from_str(&initial) {
                Ok(val) => Ok(val),
                Err(e) => {
                    cwarn!(STRATUM, "Invalid payload: '{}' ({:?})", &initial, e);
                    to_value(&[0u8; 0])
                }
            },
            None => to_value(&[0u8; 0]),
        }
        .expect("Empty slices are serializable"))
    }

    /// rpc method `mining.authorize`
    fn authorize(&self, params: Params, meta: SocketMetadata) -> RpcResult {
        params
            .parse::<(String, String)>()
            .map(|(worker_id, secret)| {
                if let Some(valid_secret) = self.secret {
                    let hash = blake256(secret);
                    if hash != valid_secret {
                        return to_value(&false)
                    }
                }
                ctrace!(STRATUM, "New worker #{} registered", worker_id);
                self.workers.write().insert(*meta.addr(), worker_id);
                to_value(true)
            })
            .map(|v| v.expect("Only true/false is returned and it's always serializable"))
    }

    /// rpc method `mining.submit`
    fn submit(&self, params: Params, meta: SocketMetadata) -> RpcResult {
        let workers = self.workers.read();
        if workers.contains_key(&meta.addr) == false {
            return Err(Error::UnauthorizedWorker.into())
        }

        params.parse::<(H256, Vec<Bytes>)>().and_then(|(pow_hash, seal)| {
            let seal = seal.iter().cloned().map(Into::into).collect();
            match self.dispatcher.submit((pow_hash, seal)) {
                Ok(()) => {
                    self.update_peers(&meta.tcp_dispatcher.expect("tcp_dispatcher is always initialized"));
                    Ok(jsonrpc_core::Value::Null)
                }
                Err(submit_err) => {
                    cwarn!(STRATUM, "Error while submitting share: {:?}", submit_err);
                    Err(submit_err.into())
                }
            }
        })
    }
}

impl StratumImpl {
    /// Helper method
    fn update_peers(&self, tcp_dispatcher: &Dispatcher) {
        if let Some(job) = self.dispatcher.job() {
            if let Err(e) = self.push_work_all(job.as_str(), tcp_dispatcher) {
                warn!("Failed to update some of the peers: {:?}", e);
            }
        }
    }

    fn push_work_all(&self, payload: &str, tcp_dispatcher: &Dispatcher) -> Result<(), Error> {
        let hup_peers = {
            let workers = self.workers.read();
            let next_request_id = {
                let mut counter = self.notify_counter.write();
                if *counter == ::std::u32::MAX {
                    *counter = NOTIFY_COUNTER_INITIAL;
                } else {
                    *counter += 1
                }
                *counter
            };

            let mut hup_peers = HashSet::with_capacity(0); // most of the cases won't be needed, hence avoid allocation
            let workers_msg =
                format!("{{ \"id\": {}, \"method\": \"mining.notify\", \"params\": {} }}", next_request_id, payload);
            ctrace!(STRATUM, "pushing work for {} workers (payload: '{}')", workers.len(), &workers_msg);
            for (ref addr, _) in workers.iter() {
                ctrace!(STRATUM, "pusing work to {}", addr);
                match tcp_dispatcher.push_message(addr, workers_msg.clone()) {
                    Err(PushMessageError::NoSuchPeer) => {
                        ctrace!(STRATUM, "Worker no longer connected: {}", &addr);
                        hup_peers.insert(**addr);
                    }
                    Err(e) => {
                        cwarn!(STRATUM, "Unexpected transport error: {:?}", e);
                    }
                    Ok(_) => {}
                }
            }
            hup_peers
        };

        if !hup_peers.is_empty() {
            let mut workers = self.workers.write();
            for hup_peer in hup_peers {
                workers.remove(&hup_peer);
            }
        }

        Ok(())
    }

    fn push_work(&self, payloads: Vec<String>, tcp_dispatcher: &Dispatcher) -> Result<(), Error> {
        if !payloads.len() > 0 {
            return Err(Error::NoWork)
        }
        let workers = self.workers.read();
        let addrs = workers.keys().collect::<Vec<&SocketAddr>>();
        if !workers.len() > 0 {
            return Err(Error::NoWorkers)
        }
        let mut que = payloads;
        let mut addr_index = 0;
        while !que.is_empty() {
            let next_worker = addrs[addr_index];
            let mut next_payload = que.drain(0..1);
            tcp_dispatcher.push_message(
                next_worker,
                next_payload.nth(0).expect("drained successfully of 0..1, so 0-th element should exist"),
            )?;
            addr_index += 1;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SocketMetadata {
    addr: SocketAddr,
    // with the new version of jsonrpc-core, SocketMetadata
    // won't have to implement default, so this field will not
    // have to be an Option
    tcp_dispatcher: Option<Dispatcher>,
}

impl Default for SocketMetadata {
    fn default() -> Self {
        SocketMetadata {
            addr: "0.0.0.0:0".parse().unwrap(),
            tcp_dispatcher: None,
        }
    }
}

impl SocketMetadata {
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }
}

impl Metadata for SocketMetadata {}

pub struct PeerMetaExtractor {
    tcp_dispatcher: Dispatcher,
}

impl PeerMetaExtractor {
    fn new(tcp_dispatcher: Dispatcher) -> Self {
        PeerMetaExtractor {
            tcp_dispatcher,
        }
    }
}

impl MetaExtractor<SocketMetadata> for PeerMetaExtractor {
    fn extract(&self, context: &RequestContext) -> SocketMetadata {
        SocketMetadata {
            addr: context.peer_addr,
            tcp_dispatcher: Some(self.tcp_dispatcher.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;

    use jsonrpc_core::futures::{future, Future};
    use primitives::{Bytes, H256};
    use tokio_core::net::TcpStream;
    use tokio_core::reactor::{Core, Timeout};
    use tokio_io::io;

    pub struct VoidManager;

    impl JobDispatcher for VoidManager {
        fn submit(&self, _payload: (H256, Vec<Bytes>)) -> Result<(), Error> {
            Ok(())
        }
    }

    fn dummy_request(addr: &SocketAddr, data: &str) -> Vec<u8> {
        let mut core = Core::new().expect("Tokio Core should be created with no errors");
        let mut buffer = vec![0u8; 2048];

        let mut data_vec = data.as_bytes().to_vec();
        data_vec.extend(b"\n");

        let stream = TcpStream::connect(addr, &core.handle())
            .and_then(|stream| io::write_all(stream, &data_vec))
            .and_then(|(stream, _)| io::read(stream, &mut buffer))
            .and_then(|(_, read_buf, len)| future::ok(read_buf[0..len].to_vec()));
        let result = core.run(stream).expect("Core should run with no errors");

        result
    }

    #[test]
    fn start() {
        let stratum = Stratum::start(&SocketAddr::from_str("127.0.0.1:19980").unwrap(), Arc::new(VoidManager), None);
        assert!(stratum.is_ok());
    }

    #[test]
    fn records_subscriber() {
        let addr = SocketAddr::from_str("127.0.0.1:19985").unwrap();
        let stratum = Stratum::start(&addr, Arc::new(VoidManager), None).unwrap();
        let request = r#"{"jsonrpc": "2.0", "method": "mining.subscribe", "params": [], "id": 1}"#;
        dummy_request(&addr, request);
        assert_eq!(1, stratum.implementation.subscribers.read().len());
    }

    struct DummyManager {
        initial_payload: String,
    }

    impl DummyManager {
        fn new() -> Arc<DummyManager> {
            Arc::new(Self::build())
        }

        fn build() -> DummyManager {
            DummyManager {
                initial_payload: r#"[ "dummy payload" ]"#.to_owned(),
            }
        }

        fn of_initial(mut self, new_initial: &str) -> DummyManager {
            self.initial_payload = new_initial.to_owned();
            self
        }
    }

    impl JobDispatcher for DummyManager {
        fn initial(&self) -> Option<String> {
            Some(self.initial_payload.clone())
        }

        fn submit(&self, _payload: (H256, Vec<Bytes>)) -> Result<(), Error> {
            Ok(())
        }
    }

    fn terminated_str(origin: &'static str) -> String {
        let mut s = String::new();
        s.push_str(origin);
        s.push_str("\n");
        s
    }

    #[test]
    fn receives_initial_paylaod() {
        let addr = SocketAddr::from_str("127.0.0.1:19975").unwrap();
        let _stratum =
            Stratum::start(&addr, DummyManager::new(), None).expect("There should be no error starting stratum");
        let request = r#"{"jsonrpc": "2.0", "method": "mining.subscribe", "params": [], "id": 2}"#;

        let response = String::from_utf8(dummy_request(&addr, request)).unwrap();

        assert_eq!(terminated_str(r#"{"jsonrpc":"2.0","result":["dummy payload"],"id":2}"#), response);
    }

    #[test]
    fn authorize() {
        let addr = SocketAddr::from_str("127.0.0.1:19970").unwrap();
        let stratum =
            Stratum::start(&addr, Arc::new(DummyManager::build().of_initial(r#"["dummy authorize payload"]"#)), None)
                .expect("There should be no error starting stratum");

        let request = r#"{"jsonrpc": "2.0", "method": "mining.authorize", "params": ["miner1", ""], "id": 1}"#;
        let response = String::from_utf8(dummy_request(&addr, request)).unwrap();

        assert_eq!(terminated_str(r#"{"jsonrpc":"2.0","result":true,"id":1}"#), response);
        assert_eq!(1, stratum.implementation.workers.read().len());
    }

    #[test]
    fn push_work() {
        let addr = SocketAddr::from_str("127.0.0.1:19995").unwrap();
        let stratum =
            Stratum::start(&addr, Arc::new(DummyManager::build().of_initial(r#"["dummy authorize payload"]"#)), None)
                .expect("There should be no error starting stratum");

        let mut auth_request = r#"{"jsonrpc": "2.0", "method": "mining.authorize", "params": ["miner1", ""], "id": 1}"#
            .as_bytes()
            .to_vec();
        auth_request.extend(b"\n");

        let mut core = Core::new().expect("Tokio Core should be created with no errors");
        let timeout1 = Timeout::new(::std::time::Duration::from_millis(100), &core.handle())
            .expect("There should be a timeout produced in message test");
        let timeout2 = Timeout::new(::std::time::Duration::from_millis(100), &core.handle())
            .expect("There should be a timeout produced in message test");
        let mut buffer = vec![0u8; 2048];
        let mut buffer2 = vec![0u8; 2048];
        let stream = TcpStream::connect(&addr, &core.handle())
            .and_then(|stream| io::write_all(stream, &auth_request))
            .and_then(|(stream, _)| io::read(stream, &mut buffer))
            .and_then(|(stream, ..)| {
                ctrace!(STRATUM, "Received authorization confirmation");
                timeout1.join(future::ok(stream))
            })
            .and_then(|(_, stream)| {
                ctrace!(STRATUM, "Pusing work to peers");
                stratum
                    .push_work_all(r#"{ "00040008", "100500" }"#.to_owned())
                    .expect("Pushing work should produce no errors");
                timeout2.join(future::ok(stream))
            })
            .and_then(|(_, stream)| {
                ctrace!(STRATUM, "Ready to read work from server");
                io::read(stream, &mut buffer2)
            })
            .and_then(|(_, read_buf, len)| {
                ctrace!(STRATUM, "Received work from server");
                future::ok(read_buf[0..len].to_vec())
            });
        let response = String::from_utf8(core.run(stream).expect("Core should run with no errors"))
            .expect("Response should be utf-8");

        assert_eq!(
            "{ \"id\": 17, \"method\": \"mining.notify\", \"params\": { \"00040008\", \"100500\" } }\n",
            response
        );
    }

    #[test]
    fn respond_to_submition() {
        let addr = SocketAddr::from_str("127.0.0.1:19990").unwrap();
        let _stratum =
            Stratum::start(&addr, Arc::new(DummyManager::build().of_initial(r#"["dummy authorize payload"]"#)), None)
                .expect("There should be no error starting stratum");

        let mut auth_request = r#"{"jsonrpc": "2.0", "method": "mining.authorize", "params": ["miner1", ""], "id": 1}"#
            .as_bytes()
            .to_vec();
        auth_request.extend(b"\n");

        let mut submit_request =
            r#"{"jsonrpc": "2.0", "method": "mining.submit", "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"]], "id": 2}"#.as_bytes()
                .to_vec();
        submit_request.extend(b"\n");

        let mut core = Core::new().expect("Tokio Core should be created with no errors");
        let timeout1 = Timeout::new(::std::time::Duration::from_millis(100), &core.handle())
            .expect("There should be a timeout produced in message test");
        let mut buffer = vec![0u8; 2048];
        let mut buffer2 = vec![0u8; 2048];
        let stream = TcpStream::connect(&addr, &core.handle())
            .and_then(|stream| io::write_all(stream, &auth_request))
            .and_then(|(stream, _)| io::read(stream, &mut buffer))
            .and_then(|(stream, ..)| {
                ctrace!(STRATUM, "Received authorization confirmation");
                timeout1.join(future::ok(stream))
            })
            .and_then(|(_, stream)| io::write_all(stream, &submit_request))
            .and_then(|(stream, _)| io::read(stream, &mut buffer2))
            .and_then(|(_, read_buf, len)| {
                ctrace!(STRATUM, "Received work from server");
                future::ok(read_buf[0..len].to_vec())
            });

        let response = String::from_utf8(core.run(stream).expect("Core should run with no errors"))
            .expect("Response should be utf-8");
        assert_eq!("{\"jsonrpc\":\"2.0\",\"result\":null,\"id\":2}\n", response);
    }

    #[test]
    fn return_error_when_unauthorized_worker_submits() {
        let addr = SocketAddr::from_str("127.0.0.1:19991").unwrap();
        let _stratum =
            Stratum::start(&addr, Arc::new(DummyManager::build().of_initial(r#"["dummy authorize payload"]"#)), None)
                .expect("There should be no error starting stratum");

        let mut submit_request =
            r#"{"jsonrpc": "2.0", "method": "mining.submit", "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"]], "id": 2}"#.as_bytes()
                .to_vec();
        submit_request.extend(b"\n");

        let mut core = Core::new().expect("Tokio Core should be created with no errors");
        let mut buffer = vec![0u8; 2048];
        let stream = TcpStream::connect(&addr, &core.handle())
            .and_then(|stream| io::write_all(stream, &submit_request))
            .and_then(|(stream, _)| io::read(stream, &mut buffer))
            .and_then(|(_, read_buf, len)| {
                ctrace!(STRATUM, "Received result from server");
                future::ok(read_buf[0..len].to_vec())
            });

        let response = String::from_utf8(core.run(stream).expect("Core should run with no errors"))
            .expect("Response should be utf-8");
        assert_eq!(
            "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":23,\"message\":\"Unauthorized worker\"},\"id\":2}\n",
            response
        );
    }
}
