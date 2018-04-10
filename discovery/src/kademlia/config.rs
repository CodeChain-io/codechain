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

use super::NodeId;

pub struct Config {
    pub node_id: Option<NodeId>,
    pub alpha: u8,
    pub k: u8,
    pub t_refresh: u32,
}

use super::ALPHA;
use super::K;
use super::T_REFRESH;

impl Config {
    pub fn new(node_id: Option<NodeId>, alpha: Option<u8>, k: Option<u8>, t_refresh: Option<u32>) -> Self {
        let alpha = alpha.unwrap_or(ALPHA);
        let k = k.unwrap_or(K);
        let t_refresh = t_refresh.unwrap_or(T_REFRESH);

        Self {
            node_id,
            alpha,
            k,
            t_refresh,
        }
    }
}
