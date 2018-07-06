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

extern crate hyper;

use std::io::Write;

use ctypes::{H256, U256};
use parking_lot::Mutex;

use self::hyper::client::{Client, Request, Response};
use self::hyper::header::ContentType;
use self::hyper::method::Method;
use self::hyper::net::HttpStream;
use self::hyper::{Next, Url};

/// Trait for notifying about new mining work
pub trait NotifyWork: Send + Sync {
    /// Fired when new mining job available
    fn notify(&self, pow_hash: H256, target: U256);
}

/// POSTs info about new work to given urls.
pub struct WorkPoster {
    urls: Vec<Url>,
    client: Mutex<Client<PostHandler>>,
}

impl WorkPoster {
    /// Create new `WorkPoster`.
    pub fn new(urls: &[String]) -> Self {
        let urls = urls.into_iter()
            .filter_map(|u| match Url::parse(u) {
                Ok(url) => Some(url),
                Err(e) => {
                    cwarn!(MINER, "Error parsing URL {} : {}", u, e);
                    None
                }
            })
            .collect();
        let client = WorkPoster::create_client();
        WorkPoster {
            client: Mutex::new(client),
            urls,
        }
    }

    fn create_client() -> Client<PostHandler> {
        Client::<PostHandler>::configure().keep_alive(true).build().expect("Error creating HTTP client")
    }
}

impl NotifyWork for WorkPoster {
    fn notify(&self, pow_hash: H256, target: U256) {
        let body = format!(r#"{{ "result": ["0x{:x}","0x{:x}"] }}"#, pow_hash, target);
        let mut client = self.client.lock();
        for u in &self.urls {
            if let Err(e) = client.request(
                u.clone(),
                PostHandler {
                    body: body.clone(),
                },
            ) {
                cwarn!(MINER, "Error sending HTTP notification to {} : {}, retrying", u, e);
                // TODO: remove this once https://github.com/hyperium/hyper/issues/848 is fixed
                *client = WorkPoster::create_client();
                if let Err(e) = client.request(
                    u.clone(),
                    PostHandler {
                        body: body.clone(),
                    },
                ) {
                    cwarn!(MINER, "Error sending HTTP notification to {} : {}", u, e);
                }
            }
        }
    }
}

struct PostHandler {
    body: String,
}

impl hyper::client::Handler<HttpStream> for PostHandler {
    fn on_request(&mut self, request: &mut Request) -> Next {
        request.set_method(Method::Post);
        request.headers_mut().set(ContentType::json());
        Next::write()
    }

    fn on_request_writable(&mut self, encoder: &mut hyper::Encoder<HttpStream>) -> Next {
        if let Err(e) = encoder.write_all(self.body.as_bytes()) {
            ctrace!(MINER, "Error posting work data: {}", e);
        }
        encoder.close();
        Next::read()
    }

    fn on_response(&mut self, _response: Response) -> Next {
        Next::end()
    }

    fn on_response_readable(&mut self, _decoder: &mut hyper::Decoder<HttpStream>) -> Next {
        Next::end()
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        ctrace!(MINER, "Error posting work data: {}", err);
        Next::end()
    }
}
