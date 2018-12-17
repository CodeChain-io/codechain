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

use ckey::{NetworkId, PlatformAddress};
use cstate::Text as TextType;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub content: String,
    pub certifier: PlatformAddress,
}

impl Text {
    pub fn from_core(from: TextType, network_id: NetworkId) -> Self {
        Self {
            content: from.content().to_string(),
            certifier: PlatformAddress::new_v1(network_id, *from.certifier()),
        }
    }
}
