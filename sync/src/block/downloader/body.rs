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

use std::collections::{HashMap, HashSet};

use ccore::UnverifiedParcel;
use ctypes::H256;

use super::super::message::RequestMessage;

pub struct BodyDownloader {
    targets: Vec<(H256, H256)>,
    downloading: HashSet<H256>,
    downloaded: HashMap<H256, Vec<UnverifiedParcel>>,
}

impl BodyDownloader {
    pub fn new(targets: Vec<(H256, H256)>) -> Self {
        Self {
            targets,
            downloading: HashSet::new(),
            downloaded: HashMap::new(),
        }
    }

    pub fn create_request(&mut self) -> Option<RequestMessage> {
        unimplemented!()
    }

    pub fn import_bodies(&mut self, hashes: Vec<H256>, bodies: Vec<Vec<UnverifiedParcel>>) {
        unimplemented!()
    }

    pub fn add_target(&mut self, targets: Vec<(H256, H256)>) {
        unimplemented!()
    }

    pub fn remove_target(&mut self, targets: Vec<H256>) {
        unimplemented!()
    }

    pub fn drain(&mut self) -> Vec<(H256, Vec<UnverifiedParcel>)> {
        unimplemented!()
    }
}
