use std::io;
use std::io::Error as IoError;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use cbytes::ToPretty;
use ccore::{Asset, Invoice, UnverifiedTransaction};
use ctypes::hash::FromHexError;
use ctypes::hash::H256;

use futures::*;
use jsonrpc_core::request::MethodCall;
use jsonrpc_core::{Id, Params, Version};
use jsonrpc_http_server::hyper::client::HttpConnector;
use jsonrpc_http_server::hyper::error::Error as HttpError;
use jsonrpc_http_server::hyper::error::UriError;
use jsonrpc_http_server::hyper::header::{ContentLength, ContentType};
use jsonrpc_http_server::hyper::{Client, Method, Request, Uri};
use jsonrpc_http_server::tokio_core::reactor::Core;
use serde_json::{self as json, Value as JsonValue};

pub trait RpcClient {
    fn ping(&mut self) -> Result<String, RpcError>;
    fn send_signed_transaction(&mut self, t: UnverifiedTransaction) -> Result<H256, RpcError>;
    fn get_transaction_invoice(&mut self, hash: H256) -> Result<Option<Invoice>, RpcError>;
    fn get_asset_scheme(&mut self, hash: H256) -> Result<Option<Asset>, RpcError>;
}

pub struct RpcHttp {
    counter: AtomicUsize,
    core: Core,
    client: Client<HttpConnector>,
    url: Uri,
}

impl RpcHttp {
    pub fn new(url: &str) -> Result<Self, RpcError> {
        let core = Core::new()?;
        let client = Client::new(&core.handle());
        let url = url.parse()?;
        Ok(RpcHttp {
            counter: ATOMIC_USIZE_INIT,
            core,
            client,
            url,
        })
    }

    fn send(&mut self, method: &'static str, params: Vec<JsonValue>) -> Result<JsonValue, RpcError> {
        let method_call = MethodCall {
            jsonrpc: Some(Version::V2),
            method: method.to_owned(),
            params: Some(Params::Array(params)),
            id: Id::Num(self.counter.fetch_add(1, Ordering::Relaxed) as u64),
        };
        let body = json::to_string(&method_call).unwrap();
        let mut request = Request::new(Method::Post, self.url.clone());
        request.headers_mut().set(ContentType::json());
        request.headers_mut().set(ContentLength(body.len() as u64));
        request.set_body(body);
        let work = self.client.request(request).and_then(|res| {
            res.body()
                .concat2()
                // FIXME: JsonError cannot converted to HttpError here
                .and_then(move |body| Ok(json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?))
        });
        self.core.run(work.map_err(From::from))
    }
}

impl RpcClient for RpcHttp {
    fn ping(&mut self) -> Result<String, RpcError> {
        let v = self.send("ping", vec![])?;
        let result = v["result"].as_str().ok_or_else(|| RpcError::ApiError(v["result"].to_string()))?;
        Ok(result.to_string())
    }

    fn send_signed_transaction(&mut self, t: UnverifiedTransaction) -> Result<H256, RpcError> {
        let encoded = ::rlp::encode(&t).to_hex();
        let v = self.send("chain_sendSignedTransaction", vec![format!("0x{}", encoded).into()])?;
        let result = v["result"].as_str().ok_or_else(|| RpcError::ApiError(v.to_string()))?;
        Ok(H256::from_str(&result[2..])?)
    }

    fn get_transaction_invoice(&mut self, hash: H256) -> Result<Option<Invoice>, RpcError> {
        let v = self.send("chain_getTransactionInvoice", vec![format!("0x{:?}", hash).into()])?;
        let invoice: Option<Invoice> = ::serde_json::from_value(v["result"].clone())
            .map_err(|e| RpcError::ApiError(format!("Failed to deserialize {:?}", e)))?;
        Ok(invoice)
    }

    fn get_asset_scheme(&mut self, hash: H256) -> Result<Option<Asset>, RpcError> {
        let v = self.send("chain_getAssetScheme", vec![format!("0x{:?}", hash).into()])?;
        let asset: Option<Asset> = ::serde_json::from_value(v["result"].clone())
            .map_err(|e| RpcError::ApiError(format!("Failed to deserialize {:?}", e)))?;
        Ok(asset)
    }
}

#[derive(Debug)]
pub enum RpcError {
    WrongVersion(String),
    MalformedResponse(String),
    ApiError(String),
    IoError(IoError),
    UriError(UriError),
    HttpError(HttpError),
    FromHexError(FromHexError),
}

impl From<IoError> for RpcError {
    fn from(err: IoError) -> RpcError {
        RpcError::IoError(err)
    }
}

impl From<UriError> for RpcError {
    fn from(err: UriError) -> RpcError {
        RpcError::UriError(err)
    }
}

impl From<HttpError> for RpcError {
    fn from(err: HttpError) -> RpcError {
        RpcError::HttpError(err)
    }
}

impl From<FromHexError> for RpcError {
    fn from(err: FromHexError) -> RpcError {
        RpcError::FromHexError(err)
    }
}
