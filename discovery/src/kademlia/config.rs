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

pub struct Config {
    pub k: u8,
    pub t_refresh: u32,
}

use super::K;
use super::T_REFRESH;

impl Config {
    pub fn new(k: Option<u8>, t_refresh: Option<u32>) -> Self {
        let k = k.unwrap_or(K);
        let t_refresh = t_refresh.unwrap_or(T_REFRESH);

        Self {
            k,
            t_refresh,
        }
    }
}
