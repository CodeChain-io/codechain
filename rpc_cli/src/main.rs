extern crate codechain_bytes as cbytes;
extern crate codechain_core as ccore;
extern crate codechain_crypto as ccrypto;
extern crate codechain_keys as ckeys;
extern crate codechain_rpc_client;
extern crate codechain_types as ctypes;
extern crate rlp;
extern crate serde_json;

#[macro_use]
extern crate clap;

use std::fs::File;
use std::io::Read;
use std::process;

use cbytes::Bytes;
use ccore::{AssetOutPoint, AssetTransferInput, AssetTransferOutput, Parcel, Transaction, UnverifiedParcel};
use ckeys::hex::FromHex;
use ckeys::{KeyPair, Private, Secret};
use codechain_rpc_client::client::{RpcClient, RpcError, RpcHttp};
use ctypes::{H160, H256, U256};

use clap::App;
use serde_json::Value;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let rpc_url = matches.value_of("rpc-server").unwrap_or("http://localhost:8080/");
    let mut rpc = match RpcHttp::new(rpc_url) {
        Ok(rpc) => rpc,
        Err(e) => {
            println!("Failed to connect RPC server: {:?}", e);
            process::exit(0);
        }
    };

    let json = match matches.value_of("commands-file") {
        Some(filename) => match load_json(filename) {
            Ok(json) => json,
            Err(e) => {
                println!("Error while loading JSON file: {}", e);
                process::exit(0);
            }
        },
        None => {
            println!("JSON file must be provided. See command-examples.json");
            process::exit(0);
        }
    };

    if !json.is_array() {
        println!("The top-level JSON object must be an array.");
        process::exit(0);
    }

    for command in json.as_array().unwrap().iter() {
        let is_name_str = command["name"].is_string();
        let is_data_obj = command["data"].is_object();
        let result = match (is_name_str, is_data_obj) {
            (false, _) => continue,
            (_, false) => continue,
            (true, true) => handle_command(&mut rpc, &command["name"], &command["data"]),
        };
        if let Err(err) = result {
            println!("CommandError: {:?}", err);
        }
    }
}

fn load_json(filename: &str) -> Result<Value, &'static str> {
    let mut f = File::open(filename).expect("File not found");
    let mut json_string = String::new();
    f.read_to_string(&mut json_string).expect("File went wrong");
    serde_json::from_str(json_string.as_ref()).map_err(|_| "Failed to parse string")
}

#[derive(Debug)]
pub enum CommandError {
    RpcError(RpcError),
    InvalidData,
    UnknownCommand,
}

impl From<RpcError> for CommandError {
    fn from(err: RpcError) -> CommandError {
        CommandError::RpcError(err)
    }
}

fn handle_command(rpc: &mut RpcClient, name: &Value, data: &Value) -> Result<(), CommandError> {
    match name.as_str().unwrap() {
        "ping" => rpc.ping()
            .map(|message| {
                println!("{}", message);
                ()
            })
            .map_err(|e| CommandError::RpcError(e)),
        "account_getAddressFromPrivate" => {
            let private: Private = get_h256(&data["private"])?.into();
            let keypair = KeyPair::from_private(private).map_err(|_| CommandError::InvalidData)?;
            println!("Address: {:?}", keypair.address());
            Ok(())
        }
        "chain_getAssetScheme" => {
            let transaction_hash = get_h256(&data["transaction_hash"])?;
            rpc.get_asset_scheme(transaction_hash)
                .map(|message| {
                    println!("{:?}", message);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        "chain_getAsset" => {
            let transaction_hash = get_h256(&data["transaction_hash"])?;
            let index = data["index"].as_u64().ok_or(CommandError::InvalidData)? as usize;
            rpc.get_asset(transaction_hash, index)
                .map(|message| {
                    println!("{:?}", message);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        "chain_sendSignedParcel" => {
            let parcel = get_unverified_parcel(data)?;
            rpc.send_signed_parcel(parcel)
                .map(|hash| {
                    println!("TxHash: 0x{:?}", hash);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        "chain_getParcelInvoice" => {
            let hash: H256 = get_h256(&data["hash"])?;
            rpc.get_parcel_invoice(hash)
                .map(|invoice| {
                    println!("{:?}", invoice);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        "devel_getStateTrieKeys" => {
            let offset = data["offset"].as_u64().unwrap() as usize;
            let limit = data["limit"].as_u64().unwrap() as usize;
            rpc.get_state_trie_keys(offset, limit)
                .map(|keys| {
                    println!("{} keys : {:?}", keys.len(), keys);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        "devel_getStateTrieValue" => {
            let key: H256 = get_h256(&data["key"])?;
            rpc.get_state_trie_value(key)
                .map(|value| {
                    println!("{:?}", value);
                    ()
                })
                .map_err(|e| CommandError::RpcError(e))
        }
        _ => Err(CommandError::UnknownCommand),
    }
}

fn get_unverified_parcel(data: &Value) -> Result<UnverifiedParcel, CommandError> {
    let secret: Secret = get_h256(&data["secret"])?;
    let nonce: U256 = get_u256(&data["nonce"])?;
    let fee: U256 = get_u256(&data["fee"])?;
    let transaction = get_transaction(&data["transaction"])?;
    let network_id: u64 = data["network_id"]
        .as_str()
        .ok_or_else(|| CommandError::InvalidData)?
        .parse()
        .map_err(|_| CommandError::InvalidData)?;
    let (unverified_parcel, _address, _) = Parcel {
        nonce,
        fee,
        transaction,
        network_id,
    }.sign(&secret.into())
        .deconstruct();
    Ok(unverified_parcel)
}

fn get_transaction(data: &Value) -> Result<Transaction, CommandError> {
    let transaction_type = data["type"].as_str().ok_or_else(|| CommandError::InvalidData)?;
    match transaction_type {
        "noop" => Ok(Transaction::Noop),
        "payment" => {
            let address: H160 = get_h160(&data["data"]["address"])?;
            let value: U256 = get_u256(&data["data"]["value"])?;
            Ok(Transaction::Payment {
                address,
                value,
            })
        }
        "mint" => {
            let data = &data["data"];
            let metadata: String = data["metadata"].as_str().ok_or(CommandError::InvalidData)?.to_string();
            let lock_script_hash: H256 = get_h256(&data["lock_script_hash"])?;
            let parameters: Vec<Bytes> = vec![]; // FIXME: I didn't implement it since rpc_cli will be replaced with JS.
            let amount = data["amount"].as_u64();
            let registrar: Option<H160> = if data["registrar"].is_string() {
                Some(get_h160(&data["registrar"])?)
            } else {
                None
            };
            Ok(Transaction::AssetMint {
                metadata,
                lock_script_hash,
                parameters,
                amount,
                registrar,
            })
        }
        "transfer" => {
            let data = &data["data"];
            let network_id = data["network_id"]
                .as_str()
                .ok_or(CommandError::InvalidData)?
                .parse()
                .map_err(|_| CommandError::InvalidData)?;
            let inputs = {
                let mut result = vec![];
                for input in data["inputs"].as_array().unwrap_or_else(|| unreachable!()) {
                    result.push(get_transfer_input(input));
                }
                result
            };
            let outputs = {
                let mut result = vec![];
                for output in data["outputs"].as_array().unwrap_or_else(|| unreachable!()) {
                    result.push(get_transfer_output(output));
                }
                result
            };
            Ok(Transaction::AssetTransfer {
                network_id,
                inputs,
                outputs,
            })
        }
        _ => Err(CommandError::UnknownCommand),
    }
}

fn get_transfer_output(data: &Value) -> AssetTransferOutput {
    let lock_script_hash = get_h256(&data["lock_script_hash"]).unwrap_or_else(|_| unreachable!());
    let parameters = {
        // FIXME
        vec![]
    };
    let asset_type = get_h256(&data["asset_type"]).unwrap_or_else(|_| unreachable!());
    let amount = data["amount"].as_u64().unwrap();
    AssetTransferOutput {
        lock_script_hash,
        parameters,
        asset_type,
        amount,
    }
}

fn get_transfer_input(data: &Value) -> AssetTransferInput {
    let prev_out = {
        let ref data = data["prev_out"];
        let transaction_hash = get_h256(&data["transaction_hash"]).unwrap_or_else(|_| unreachable!());
        let index = data["index"].as_u64().unwrap_or_else(|| unreachable!()) as usize;
        let asset_type = get_h256(&data["asset_type"]).unwrap_or_else(|_| unreachable!());
        let amount = data["amount"].as_u64().unwrap();
        AssetOutPoint {
            transaction_hash,
            index,
            asset_type,
            amount,
        }
    };
    let lock_script = data["lock_script"].as_str().unwrap_or_else(|| unreachable!()).to_string().into_bytes();
    let unlock_script = data["unlock_script"].as_str().unwrap_or_else(|| unreachable!()).to_string().into_bytes();
    AssetTransferInput {
        prev_out,
        lock_script,
        unlock_script,
    }
}

fn get_u256(data: &Value) -> Result<U256, CommandError> {
    let val = data.as_str().ok_or_else(|| CommandError::InvalidData)?;
    // NOTE: from_hex() requires the length to be multiple of 2
    let val = if val.len() % 2 == 1 {
        format!("0{}", val)
    } else {
        String::from(val)
    };
    val.from_hex().map(|v| U256::from(v.as_slice())).map_err(|_| CommandError::InvalidData)
}

fn get_h256(data: &Value) -> Result<H256, CommandError> {
    let val = data.as_str().ok_or_else(|| CommandError::InvalidData)?;
    if val.len() != 64 {
        return Err(CommandError::InvalidData)
    }
    val.from_hex().map(|v| H256::from(v.as_slice())).map_err(|_| CommandError::InvalidData)
}

fn get_h160(data: &Value) -> Result<H160, CommandError> {
    let val = data.as_str().ok_or_else(|| CommandError::InvalidData)?;
    if val.len() != 40 {
        return Err(CommandError::InvalidData)
    }
    val.from_hex().map(|v| H160::from(v.as_slice())).map_err(|_| CommandError::InvalidData)
}
