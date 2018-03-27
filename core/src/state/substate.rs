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

//! Execution environment substate.
use std::collections::HashSet;
use ethereum_types::{U256, Address};
use log_entry::LogEntry;
use evm::{Schedule, CleanDustMode};
use super::CleanupMode;

/// State changes which should be applied in finalize,
/// after transaction is fully executed.
#[derive(Debug, Default)]
pub struct Substate {
	/// Any accounts that have suicided.
	pub suicides: HashSet<Address>,

	/// Any accounts that are touched.
	pub touched: HashSet<Address>,

	/// Any logs.
	pub logs: Vec<LogEntry>,

	/// Refund counter of SSTORE nonzero -> zero.
	pub sstore_clears_count: U256,

	/// Created contracts.
	pub contracts_created: Vec<Address>,
}

impl Substate {
	/// Creates new substate.
	pub fn new() -> Self {
		Substate::default()
	}

	/// Merge secondary substate `s` into self, accruing each element correspondingly.
	pub fn accrue(&mut self, s: Substate) {
		self.suicides.extend(s.suicides);
		self.touched.extend(s.touched);
		self.logs.extend(s.logs);
		self.sstore_clears_count = self.sstore_clears_count + s.sstore_clears_count;
		self.contracts_created.extend(s.contracts_created);
	}

	/// Get the cleanup mode object from this.
	pub fn to_cleanup_mode(&mut self, schedule: &Schedule) -> CleanupMode {
		match (schedule.kill_dust != CleanDustMode::Off, schedule.no_empty, schedule.kill_empty) {
			(false, false, _) => CleanupMode::ForceCreate,
			(false, true, false) => CleanupMode::NoEmpty,
			(false, true, true) | (true, _, _,) => CleanupMode::TrackTouched(&mut self.touched),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::Substate;
	use log_entry::LogEntry;

	#[test]
	fn created() {
		let sub_state = Substate::new();
		assert_eq!(sub_state.suicides.len(), 0);
	}

	#[test]
	fn accrue() {
		let mut sub_state = Substate::new();
		sub_state.contracts_created.push(1u64.into());
		sub_state.logs.push(LogEntry {
			address: 1u64.into(),
			topics: vec![],
			data: vec![]
		});
		sub_state.sstore_clears_count = 5.into();
		sub_state.suicides.insert(10u64.into());

		let mut sub_state_2 = Substate::new();
		sub_state_2.contracts_created.push(2u64.into());
		sub_state_2.logs.push(LogEntry {
			address: 1u64.into(),
			topics: vec![],
			data: vec![]
		});
		sub_state_2.sstore_clears_count = 7.into();

		sub_state.accrue(sub_state_2);
		assert_eq!(sub_state.contracts_created.len(), 2);
		assert_eq!(sub_state.sstore_clears_count, 12.into());
		assert_eq!(sub_state.suicides.len(), 1);
	}
}
