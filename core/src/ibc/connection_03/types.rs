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

pub enum ConnectionState {
    INIT,
    TRYOPEN,
    OPEN,
}

pub enum ConnectionVersion {
    Pick(String),
    Compatible(Vec<String>),
}

pub struct ConnectionEnd {
    state: ConnectionState,
    counterparty_connection_id: String,
    // NOTE: counterparty_prefix is required according to the spec.
    client_id: String,
    counterparty_client_id: String,
    version: ConnectionVersion,
}
