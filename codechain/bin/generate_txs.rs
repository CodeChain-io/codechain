extern crate codechain_core as ccore;
extern crate codechain_crypto as ccrypto;
extern crate codechain_key as ckey;
extern crate codechain_types as ctypes;
extern crate rlp;
extern crate rustc_hex;
#[macro_use]
extern crate jsonrpc_client_core;
extern crate clap;
extern crate jsonrpc_client_http;

use ccore::SignedTransaction;
use ccrypto::blake256;
use ckey::{sign, KeyPair, NetworkId, PlatformAddress, Private};
use clap::clap_app;
use ctypes::transaction::{Action, Transaction};
use jsonrpc_client_http::HttpTransport;
use rustc_hex::ToHex;

jsonrpc_client!(pub struct RpcClient {
    pub fn chain_getSeq(&mut self, address: PlatformAddress) -> RpcRequest<u64>;
});

fn main() {
    let matches = clap_app!(txGenerator =>
        (version: "1.0")
        (about: "Generate store transactions")
        (@arg RPC_URL: -u --rpcurl +takes_value "CodeChain RPC address like http://127.0.0.1:7070")
        (@arg SECRET: -s --secret +takes_value "Secret key of sender")
        (@arg CONTENT: -c --content +takes_value "Content of store tx")
        (@arg NETWORK_ID: --networkid +takes_value "network id")
        (@arg TX_COUNT: -n --txcount +takes_value "transaction count")
    )
    .get_matches();

    let rpc_url = matches.value_of("RPC_URL").unwrap_or("http://127.0.0.1:7070").to_string();
    let secret: Private = {
        let secret_string = matches
            .value_of("SECRET")
            .unwrap_or("eeec7e5fc8af6fcfbd728de615d8a79b1b0bbcaabd8b9bae0a842c6f708e2c0c")
            .to_string();
        secret_string.parse().expect("parse private key")
    };
    let content = matches.value_of("CONTENT").unwrap_or("dummy_content").to_string();
    let network_id: NetworkId = {
        let network_id_str = matches.value_of("NETWORK_ID").unwrap_or("bw");
        network_id_str.parse().expect("parse network id")
    };
    let tx_count: u64 = {
        let tx_count_str = matches.value_of("TX_COUNT").unwrap_or("10000");
        tx_count_str.parse().expect("parse tx count")
    };

    let keypair = KeyPair::from_private(secret).expect("create private key");
    let address = keypair.address();
    let platform_address = PlatformAddress::new_v1(network_id, address);
    eprintln!("platform address {}", platform_address);

    let transport = HttpTransport::new().standalone().unwrap();
    let transport_handle = transport.handle(&rpc_url).unwrap();
    let mut client = RpcClient::new(transport_handle);

    let seq = client.chain_getSeq(platform_address).call().expect("getseq");
    eprintln!("seq {}", seq);

    let message = blake256(&rlp::encode(&content));
    let signature = sign(keypair.private(), &message).expect("sign");

    let mut txs = Vec::new();
    for i in 0..tx_count {
        let tx = Transaction {
            seq: seq + i,
            fee: 0,
            network_id,
            action: Action::Store {
                content: content.clone(),
                certifier: keypair.address(),
                signature,
            },
        };
        let signed_tx = SignedTransaction::new_with_sign(tx, keypair.private());
        let serialized = rlp::encode(&signed_tx);
        let hex_tx = serialized.to_hex();
        txs.push(hex_tx);
    }

    let serialized_txs = serde_json::to_string(&txs).expect("json serialize");
    println!("{}", serialized_txs);
    eprintln!("{} transaction created", tx_count);
}
