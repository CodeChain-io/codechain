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


use cio::{ IoContext, IoHandler, TimerToken, StreamToken };
use mio::deprecated::EventLoop;
use parking_lot::Mutex;

use super::Handshake;
use super::HandshakeMessage;
use super::super::Address;

pub struct HandshakeHandler {
    address: Address,
    handshake: Mutex<Option<Handshake>>,
}

impl HandshakeHandler {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            handshake: Mutex::new(None),
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    Bind,
}

const RECV_TOKEN: usize = 0;
const RECV_MS: u64 = 1000;

impl IoHandler<HandlerMessage> for HandshakeHandler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) {
        io.message(HandlerMessage::Bind).expect("Cannot run UDP io service");
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) {
        info!("handshake server closed");
        *self.handshake.lock() = None;
    }

    fn timeout(&self, _io: &IoContext<HandlerMessage>, token: TimerToken) {
        match token {
            RECV_TOKEN => {
                loop {
                    if let Some(handshake) = self.handshake.lock().as_ref() {
                        match handshake.receive() {
                            Ok(None) => {
                                break;
                            },
                            Ok(Some((msg, address))) => {
                                handshake.on_packet(&msg, &address);
                            },
                            Err(err) => {
                                info!("handshake receive error {}", err);
                            },
                        };
                    };
                };
            },
            _ => {
                info!("Unknown timer token {}", token);
            },
        };
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) {
        match message {
            &HandlerMessage::Bind => {
                info!("Handshake service bind to {:?}", &self.address);
                let handshake = Handshake::bind(&self.address).expect("Cannot bind UDP port");
                *self.handshake.lock() = Some(handshake);
                let _ = io.register_timer(RECV_TOKEN, RECV_MS);
            },
        };
    }
}
