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

//! General IO module.
//!
//! Example usage for creating a network service and adding an IO handler:
//!
//! ```rust
//! extern crate codechain_io as cio;
//! use cio::*;
//! use std::sync::Arc;
//!
//! struct MyHandler;
//!
//! #[derive(Clone)]
//! struct MyMessage {
//! 	data: u32
//! }
//!
//! impl IoHandler<MyMessage> for MyHandler {
//! 	fn initialize(&self, io: &IoContext<MyMessage>) -> IoHandlerResult<()> {
//!			io.register_timer(0, 1000);
//!			Ok(())
//!		}
//!
//!		fn timeout(&self, _io: &IoContext<MyMessage>, timer: TimerToken) -> IoHandlerResult<()> {
//!			println!("Timeout {}", timer);
//!			Ok(())
//!		}
//!
//!		fn message(&self, _io: &IoContext<MyMessage>, message: &MyMessage) -> IoHandlerResult<()> {
//!			println!("Message {}", message.data);
//!			Ok(())
//!		}
//! }
//!
//! fn main () {
//! 	let mut service = IoService::<MyMessage>::start().expect("Error creating network service");
//! 	service.register_handler(Arc::new(MyHandler)).unwrap();
//!
//! 	// Wait for quit condition
//! 	// ...
//! 	// Drop the service
//! }
//! ```

//TODO: use Poll from mio
#![allow(deprecated)]

#[macro_use]
extern crate codechain_logger as clogger;
extern crate mio;
#[macro_use]
extern crate log;
extern crate crossbeam;
extern crate parking_lot;
extern crate slab;

mod service;
mod worker;

use mio::deprecated::{EventLoop, NotifyError};
use mio::Token;
use std::{error, fmt};

pub use worker::LOCAL_STACK_SIZE;

#[derive(Debug)]
/// IO Error
pub enum IoError {
    /// Low level error from mio crate
    Mio(::std::io::Error),
    /// Error concerning the Rust standard library's IO subsystem.
    StdIo(::std::io::Error),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // just defer to the std implementation for now.
        // we can refine the formatting when more variants are added.
        match *self {
            IoError::Mio(ref std_err) => std_err.fmt(f),
            IoError::StdIo(ref std_err) => std_err.fmt(f),
        }
    }
}

impl error::Error for IoError {
    fn description(&self) -> &str {
        "IO error"
    }
}

impl From<::std::io::Error> for IoError {
    fn from(err: ::std::io::Error) -> IoError {
        IoError::StdIo(err)
    }
}

impl<Message> From<NotifyError<service::IoMessage<Message>>> for IoError
where
    Message: Send + Clone,
{
    fn from(_err: NotifyError<service::IoMessage<Message>>) -> IoError {
        IoError::Mio(::std::io::Error::new(::std::io::ErrorKind::ConnectionAborted, "Network IO notification error"))
    }
}

#[derive(Debug)]
pub struct IoHandlerError(String);

impl<E: ToString> From<E> for IoHandlerError {
    fn from(err: E) -> Self {
        IoHandlerError(err.to_string())
    }
}

pub type IoHandlerResult<T> = Result<T, IoHandlerError>;

/// Generic IO handler.
/// All the handler function are called from within IO event loop.
/// `Message` type is used as notification data
pub trait IoHandler<Message>: Send + Sync
where
    Message: Send + Sync + Clone + 'static, {
    /// Initialize the handler
    fn initialize(&self, _io: &IoContext<Message>) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Timer function called after a timeout created with `HandlerIo::timeout`.
    fn timeout(&self, _io: &IoContext<Message>, _timer: TimerToken) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Called when a broadcasted message is received. The message can only be sent from a different IO handler.
    fn message(&self, _io: &IoContext<Message>, _message: &Message) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Called when an IO stream gets closed
    fn stream_hup(&self, _io: &IoContext<Message>, _stream: StreamToken) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Called when an IO stream can be read from
    fn stream_readable(&self, _io: &IoContext<Message>, _stream: StreamToken) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Called when an IO stream can be written to
    fn stream_writable(&self, _io: &IoContext<Message>, _stream: StreamToken) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Register a new stream with the event loop
    fn register_stream(
        &self,
        _stream: StreamToken,
        _reg: Token,
        _event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Re-register a stream with the event loop
    fn update_stream(
        &self,
        _stream: StreamToken,
        _reg: Token,
        _event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        Ok(())
    }
    /// Deregister a stream. Called whenstream is removed from event loop
    fn deregister_stream(
        &self,
        _stream: StreamToken,
        _event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        Ok(())
    }
}

pub use service::IoChannel;
pub use service::IoContext;
pub use service::IoManager;
pub use service::IoService;
pub use service::StreamToken;
pub use service::TimerToken;
pub use service::TOKENS_PER_HANDLER;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MyHandler;

    #[derive(Clone)]
    struct MyMessage {
        data: u32,
    }

    impl IoHandler<MyMessage> for MyHandler {
        fn initialize(&self, io: &IoContext<MyMessage>) -> IoHandlerResult<()> {
            io.register_timer(0, 1000);
            Ok(())
        }

        fn timeout(&self, _io: &IoContext<MyMessage>, timer: TimerToken) -> IoHandlerResult<()> {
            println!("Timeout {}", timer);
            Ok(())
        }

        fn message(&self, _io: &IoContext<MyMessage>, message: &MyMessage) -> IoHandlerResult<()> {
            println!("Message {}", message.data);
            Ok(())
        }
    }

    #[test]
    fn service_register_handler() {
        let service = IoService::<MyMessage>::start().expect("Error creating network service");
        service.register_handler(Arc::new(MyHandler)).unwrap();
    }
}
