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

use std::sync::Weak;

use cio::{IoContext, IoHandler, IoHandlerResult, TimerToken};
use time::Duration;

use super::super::machine::Machine;
use super::ConsensusEngine;

/// Timeouts lookup
pub trait Timeouts<S: Sync + Send + Clone>: Send + Sync {
    /// Return the first timeout.
    fn initial(&self) -> Duration;

    /// Get a timeout based on step.
    fn timeout(&self, step: &S) -> Duration;
}

/// Timeout transition handling.
pub struct TransitionHandler<S: Sync + Send + Clone, M: Machine> {
    engine: Weak<ConsensusEngine<M>>,
    timeouts: Box<Timeouts<S>>,
}

impl<S, M: Machine> TransitionHandler<S, M>
where
    S: Sync + Send + Clone,
{
    /// New step caller by timeouts.
    pub fn new(engine: Weak<ConsensusEngine<M>>, timeouts: Box<Timeouts<S>>) -> Self {
        TransitionHandler {
            engine,
            timeouts,
        }
    }
}

/// Timer token representing the consensus step timeouts.
pub const ENGINE_TIMEOUT_TOKEN: TimerToken = 23;

fn set_timeout<S: Sync + Send + Clone>(io: &IoContext<S>, timeout: Duration) {
    io.register_timer_once(ENGINE_TIMEOUT_TOKEN, timeout.num_milliseconds() as u64)
        .unwrap_or_else(|e| warn!(target: "engine", "Failed to set consensus step timeout: {}.", e))
}

impl<S, M> IoHandler<S> for TransitionHandler<S, M>
where
    S: Sync + Send + Clone + 'static,
    M: Machine,
{
    fn initialize(&self, io: &IoContext<S>) -> IoHandlerResult<()> {
        let initial = self.timeouts.initial();
        trace!(target: "engine", "Setting the initial timeout to {}.", initial);
        set_timeout(io, initial);
        Ok(())
    }

    /// Call step after timeout.
    fn timeout(&self, _io: &IoContext<S>, timer: TimerToken) -> IoHandlerResult<()> {
        if timer == ENGINE_TIMEOUT_TOKEN {
            if let Some(engine) = self.engine.upgrade() {
                engine.step();
            }
        }
        Ok(())
    }

    /// Set a new timer on message.
    fn message(&self, io: &IoContext<S>, next: &S) -> IoHandlerResult<()> {
        if let Err(io_err) = io.clear_timer(ENGINE_TIMEOUT_TOKEN) {
            warn!(target: "engine", "Could not remove consensus timer {}.", io_err)
        }
        set_timeout(io, self.timeouts.timeout(next));
        Ok(())
    }
}
