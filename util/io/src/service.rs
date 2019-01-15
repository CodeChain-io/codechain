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

use crossbeam::deque;
use mio::deprecated::{EventLoop, EventLoopBuilder, Handler, Sender};
use mio::timer::Timeout;
use mio::*;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::sync::{Condvar as SCondvar, Mutex as SMutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use worker::{Work, WorkType, Worker};

use super::{IoError, IoHandler};

/// Timer ID
pub type TimerToken = usize;
/// Timer ID
pub type StreamToken = usize;
/// IO Hadndler ID
pub type HandlerId = usize;

/// Maximum number of tokens a handler can use
pub const TOKENS_PER_HANDLER: usize = 16384;

/// Messages used to communicate with the event loop from other threads.
pub enum IoMessage<Message>
where
    Message: Send + Sized, {
    /// Shutdown the event loop
    Shutdown,
    AddTimer {
        token: TimerToken,
        delay: u64,
        once: bool,
    },
    RemoveTimer {
        token: TimerToken,
    },
    RegisterStream {
        token: StreamToken,
    },
    DeregisterStream {
        token: StreamToken,
    },
    UpdateStreamRegistration {
        token: StreamToken,
    },
    /// Send a message across a handler.
    UserMessage(Message),
}

/// IO access point. This is passed to IO handler and provides an interface to the IO subsystem.
pub struct IoContext<Message>
where
    Message: Send + Sync + 'static, {
    channel: IoChannel<Message>,
}

impl<Message> IoContext<Message>
where
    Message: Send + Sync + 'static,
{
    /// Create a new IO access point. Takes references to all the data that can be updated within the IO handler.
    pub fn new(channel: IoChannel<Message>) -> IoContext<Message> {
        IoContext {
            channel,
        }
    }

    /// Register a new recurring IO timer. 'IoHandler::timeout' will be called with the token.
    pub fn register_timer(&self, token: TimerToken, ms: u64) {
        self.channel
            .send_io(IoMessage::AddTimer {
                token,
                delay: ms,
                once: false,
            })
            .unwrap();
    }

    /// Register a new IO timer once. 'IoHandler::timeout' will be called with the token.
    pub fn register_timer_once(&self, token: TimerToken, ms: u64) {
        self.channel
            .send_io(IoMessage::AddTimer {
                token,
                delay: ms,
                once: true,
            })
            .unwrap();
    }

    /// Delete a timer.
    pub fn clear_timer(&self, token: TimerToken) {
        self.channel
            .send_io(IoMessage::RemoveTimer {
                token,
            })
            .unwrap();
    }

    /// Register a new IO stream.
    pub fn register_stream(&self, token: StreamToken) {
        self.channel
            .send_io(IoMessage::RegisterStream {
                token,
            })
            .unwrap();
    }

    /// Deregister an IO stream.
    pub fn deregister_stream(&self, token: StreamToken) {
        self.channel
            .send_io(IoMessage::DeregisterStream {
                token,
            })
            .unwrap();
    }

    /// Reregister an IO stream.
    pub fn update_registration(&self, token: StreamToken) {
        self.channel
            .send_io(IoMessage::UpdateStreamRegistration {
                token,
            })
            .unwrap();
    }

    /// Broadcast a message to other IO clients
    pub fn message(&self, message: Message) {
        self.channel.send(message).unwrap();
    }

    /// Get message channel
    pub fn channel(&self) -> IoChannel<Message> {
        self.channel.clone()
    }
}

#[derive(Clone)]
struct UserTimer {
    delay: u64,
    timeout: Timeout,
    once: bool,
}

type HandlerType<M> = RwLock<Option<Arc<IoHandler<M>>>>;

/// Root IO handler. Manages user handler, messages and IO timers.
pub struct IoManager<Message>
where
    Message: Send + Sync, {
    timers: Arc<RwLock<HashMap<HandlerId, UserTimer>>>,
    handler: Arc<HandlerType<Message>>,
    workers: Vec<Worker>,
    worker_channel: deque::Worker<Work<Message>>,
    work_ready: Arc<SCondvar>,
}

impl<Message> IoManager<Message>
where
    Message: Send + Sync + 'static,
{
    /// Creates a new instance and registers it with the event loop.
    pub fn start(
        event_loop: &mut EventLoop<IoManager<Message>>,
        handler: Arc<HandlerType<Message>>,
        name: &str,
    ) -> Result<(), IoError> {
        let (worker, stealer) = deque::lifo();
        let num_workers = 4;
        let work_ready_mutex = Arc::new(SMutex::new(()));
        let work_ready = Arc::new(SCondvar::new());

        let workers = (0..num_workers)
            .map(|i| {
                Worker::new(
                    i,
                    stealer.clone(),
                    IoChannel::new(event_loop.channel(), Arc::downgrade(&handler)),
                    work_ready.clone(),
                    work_ready_mutex.clone(),
                    name,
                )
            })
            .collect();

        let mut io = IoManager {
            timers: Arc::new(RwLock::new(HashMap::new())),
            handler,
            worker_channel: worker,
            workers,
            work_ready,
        };
        event_loop.run(&mut io)?;
        Ok(())
    }
}

impl<Message> Handler for IoManager<Message>
where
    Message: Send + Sync + 'static,
{
    type Timeout = Token;
    type Message = IoMessage<Message>;

    fn ready(&mut self, _event_loop: &mut EventLoop<Self>, token: Token, events: Ready) {
        let token = token.0;
        if let Some(handler) = &*self.handler.read() {
            if events.is_hup() {
                self.worker_channel.push(Work {
                    work_type: WorkType::Hup,
                    token,
                    handler: Arc::clone(&handler),
                });
            } else {
                if events.is_readable() {
                    self.worker_channel.push(Work {
                        work_type: WorkType::Readable,
                        token,
                        handler: Arc::clone(&handler),
                    });
                }
                if events.is_writable() {
                    self.worker_channel.push(Work {
                        work_type: WorkType::Writable,
                        token,
                        handler: Arc::clone(&handler),
                    });
                }
            }
            self.work_ready.notify_all();
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, token: Token) {
        if let Some(handler) = &*self.handler.read() {
            let maybe_timer = self.timers.read().get(&token.0).cloned();
            if let Some(timer) = maybe_timer {
                if timer.once {
                    self.timers.write().remove(&token.0);
                    event_loop.clear_timeout(&timer.timeout);
                } else {
                    event_loop
                        .timeout(token, Duration::from_millis(timer.delay))
                        .expect("Error re-registering user timer");
                }
                let token = token.0;
                self.worker_channel.push(Work {
                    work_type: WorkType::Timeout,
                    token,
                    handler: Arc::clone(&handler),
                });
                self.work_ready.notify_all();
            }
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Self::Message) {
        match msg {
            IoMessage::Shutdown => {
                self.workers.clear();
                event_loop.shutdown();
            }
            IoMessage::AddTimer {
                token,
                delay,
                once,
            } => {
                let timeout = event_loop
                    .timeout(Token(token), Duration::from_millis(delay))
                    .expect("Error registering user timer");
                self.timers.write().insert(
                    token,
                    UserTimer {
                        delay,
                        timeout,
                        once,
                    },
                );
            }
            IoMessage::RemoveTimer {
                token,
            } => {
                if let Some(timer) = self.timers.write().remove(&token) {
                    event_loop.clear_timeout(&timer.timeout);
                }
            }
            IoMessage::RegisterStream {
                token,
            } => {
                if let Some(handler) = &*self.handler.read() {
                    if let Err(err) = handler.register_stream(token, Token(token), event_loop) {
                        cwarn!(IO, "Error in register_stream {:?}", err);
                    }
                }
            }
            IoMessage::DeregisterStream {
                token,
            } => {
                if let Some(handler) = &*self.handler.read() {
                    if let Err(err) = handler.deregister_stream(token, event_loop) {
                        cwarn!(IO, "Error in deregister_stream {:?}", err);
                    }
                    // unregister a timer associated with the token (if any)
                    if let Some(timer) = self.timers.write().remove(&token) {
                        event_loop.clear_timeout(&timer.timeout);
                    }
                }
            }
            IoMessage::UpdateStreamRegistration {
                token,
            } => {
                if let Some(handler) = &*self.handler.read() {
                    if let Err(err) = handler.update_stream(token, Token(token), event_loop) {
                        cwarn!(IO, "Error in update_stream {:?}", err);
                    }
                }
            }
            IoMessage::UserMessage(data) => {
                if let Some(h) = &*self.handler.read() {
                    let handler = Arc::clone(&h);
                    self.worker_channel.push(Work {
                        work_type: WorkType::Message(data),
                        token: 0,
                        handler,
                    });
                }
                self.work_ready.notify_all();
            }
        }
    }
}

/// Allows sending messages into the event loop. All the IO handler will get the message
/// in the `message` callback.
pub struct IoChannel<Message>
where
    Message: Send, {
    channel: Option<Sender<IoMessage<Message>>>,
    handler: Weak<HandlerType<Message>>,
}

impl<Message> Clone for IoChannel<Message>
where
    Message: Send + Sync + 'static,
{
    fn clone(&self) -> IoChannel<Message> {
        IoChannel {
            channel: self.channel.clone(),
            handler: Weak::clone(&self.handler),
        }
    }
}

impl<Message> IoChannel<Message>
where
    Message: Send + Sync + 'static,
{
    /// Send a message through the channel
    pub fn send(&self, message: Message) -> Result<(), IoError> {
        match self.channel {
            Some(ref channel) => channel.send(IoMessage::UserMessage(message))?,
            None => self.send_sync(message)?,
        }
        Ok(())
    }

    /// Send a message through the channel and handle it synchronously
    pub fn send_sync(&self, message: Message) -> Result<(), IoError> {
        if let Some(handler) = self.handler.upgrade() {
            if let Some(h) = &*handler.read() {
                let handler = Arc::clone(&h);
                if let Err(err) = handler.message(&IoContext::new(self.clone()), &message) {
                    cwarn!(IO, "Error in message {:?}", err);
                }
            }
        }
        Ok(())
    }

    /// Send low level io message
    fn send_io(&self, message: IoMessage<Message>) -> Result<(), IoError> {
        if let Some(ref channel) = self.channel {
            channel.send(message)?
        }
        Ok(())
    }
    /// Create a new channel disconnected from an event loop.
    pub fn disconnected() -> IoChannel<Message> {
        IoChannel {
            channel: None,
            handler: Weak::default(),
        }
    }

    fn new(channel: Sender<IoMessage<Message>>, handler: Weak<HandlerType<Message>>) -> IoChannel<Message> {
        IoChannel {
            channel: Some(channel),
            handler,
        }
    }
}

/// General IO Service. Starts an event loop and dispatches IO requests.
/// 'Message' is a notification message type
pub struct IoService<Message>
where
    Message: Send + Sync + 'static, {
    thread: Mutex<Option<JoinHandle<()>>>,
    host_channel: Mutex<Sender<IoMessage<Message>>>,
    handler: Arc<HandlerType<Message>>,
    event_loop_channel: Sender<IoMessage<Message>>,
}

impl<Message> IoService<Message>
where
    Message: Send + Sync + 'static,
{
    /// Starts IO event loop
    pub fn start(name: &'static str) -> Result<IoService<Message>, IoError> {
        let mut config = EventLoopBuilder::new();
        config.messages_per_tick(1024);
        let mut event_loop = config.build().expect("Error creating event loop");
        let channel = event_loop.channel();
        let handler = Arc::new(RwLock::new(None));
        let h = Arc::clone(&handler);
        let thread = thread::spawn(move || {
            IoManager::<Message>::start(&mut event_loop, h, name).expect("Error starting IO service");
        });
        Ok(IoService {
            thread: Mutex::new(Some(thread)),
            host_channel: Mutex::new(channel.clone()),
            handler,
            event_loop_channel: channel,
        })
    }

    pub fn stop(&self) {
        ctrace!(SHUTDOWN, "[IoService] Closing...");
        // Clear handler so that shared pointers are not stuck on stack
        // in Channel::send_sync
        *self.handler.write() = None;
        self.host_channel
            .lock()
            .send(IoMessage::Shutdown)
            .unwrap_or_else(|e| cwarn!(IO, "Error on IO service shutdown: {:?}", e));
        if let Some(thread) = self.thread.lock().take() {
            thread.join().unwrap_or_else(|e| {
                cdebug!(SHUTDOWN, "Error joining IO service event loop thread: {:?}", e);
            });
        }
        ctrace!(SHUTDOWN, "[IoService] Closed.");
    }

    /// Register an IO handler with the event loop.
    pub fn register_handler(&self, handler: Arc<IoHandler<Message> + Send>) -> Result<(), IoError> {
        let h = Arc::clone(&handler);
        assert!(self.handler.read().is_none());
        *self.handler.write() = Some(handler);
        h.initialize(&IoContext::new(IoChannel::new(self.event_loop_channel.clone(), Arc::downgrade(&self.handler))))?;
        Ok(())
    }

    /// Send a message over the network. Normaly `HostIo::send` should be used. This can be used from non-io threads.
    pub fn send_message(&self, message: Message) -> Result<(), IoError> {
        self.host_channel.lock().send(IoMessage::UserMessage(message))?;
        Ok(())
    }

    /// Create a new message channel
    pub fn channel(&self) -> IoChannel<Message> {
        IoChannel::new(self.host_channel.lock().clone(), Arc::downgrade(&self.handler))
    }
}

impl<Message> Drop for IoService<Message>
where
    Message: Send + Sync,
{
    fn drop(&mut self) {
        self.stop()
    }
}
