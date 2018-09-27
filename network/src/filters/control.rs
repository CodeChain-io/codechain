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

use std::net::IpAddr;

use super::filter::FilterEntry;

pub trait Control: Send + Sync {
    fn add_to_whitelist(&self, addr: IpAddr, tag: Option<String>);
    fn remove_from_whitelist(&self, addr: &IpAddr);

    fn add_to_blacklist(&self, addr: IpAddr, tag: Option<String>);
    fn remove_from_blacklist(&self, addr: &IpAddr);

    fn enable_whitelist(&self);
    fn disable_whitelist(&self);
    fn enable_blacklist(&self);
    fn disable_blacklist(&self);

    fn get_whitelist(&self) -> (Vec<FilterEntry>, bool);
    fn get_blacklist(&self) -> (Vec<FilterEntry>, bool);

    fn is_allowed(&self, addr: &IpAddr) -> bool;
}
