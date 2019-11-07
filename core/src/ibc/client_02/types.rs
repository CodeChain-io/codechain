// Copyright 2019 Kodebox, Inc.
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

use ibc;
use ibc::commitment_23 as commitment;

pub type Kind = u8;

pub trait ConsensusState {
    fn kind(&self) -> Kind;
    fn get_height(&self) -> u64;
    fn get_root(&self) -> &dyn commitment::Root;
    fn check_validity_and_update_state(&mut self, header: &[u8]) -> Result<(), String>;
    fn check_misbehaviour_and_update_state(&mut self) -> bool;
    fn encode(&self) -> Vec<u8>;
}

pub trait Header {
    fn kind(&self) -> Kind;
    fn get_height(&self) -> u64;
    fn encode(&self) -> &[u8];
}

pub const KIND_CODECHAIN: Kind = 0_u8;

pub trait State {
    fn get_consensus_state(&self, ctx: &mut dyn ibc::Context) -> Box<dyn ConsensusState>;
    fn set_consensus_state(&self, ctx: &mut dyn ibc::Context, cs: &dyn ConsensusState);
    fn get_root(&self, ctx: &mut dyn ibc::Context, block_height: u64) -> Result<Box<dyn commitment::Root>, String>;
    fn set_root(&self, ctx: &mut dyn ibc::Context, block_height: u64, root: &dyn commitment::Root);
    fn exists(&self, ctx: &mut dyn ibc::Context) -> bool;
    fn update(&self, ctx: &mut dyn ibc::Context, header: &[u8]) -> Result<(), String>;
}
