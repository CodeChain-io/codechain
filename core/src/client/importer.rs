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

use std::sync::Arc;

use super::{Client, ClientConfig};
use super::super::consensus::CodeChainEngine;
use super::super::error::Error;
use super::super::verification::{self, Verifier};

pub struct Importer {
    /// Used to verify blocks
    pub verifier: Box<Verifier<Client>>,

    /// CodeChain engine to be used during import
    pub engine: Arc<CodeChainEngine>,
}

impl Importer {
    pub fn new(
        config: &ClientConfig,
        engine: Arc<CodeChainEngine>,
    ) -> Result<Importer, Error> {
        Ok(Importer {
            verifier: verification::new(config.verifier_type.clone()),
            engine,
        })
    }

    /// This is triggered by a message coming from a block queue when the block is ready for insertion
    pub fn import_verified_blocks(&self, client: &Client) -> usize {
        unimplemented!();
    }
}

