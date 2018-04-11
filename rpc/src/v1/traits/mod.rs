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

use ctypes::H256;

use jsonrpc_core::Result;

use super::types::Bytes;

build_rpc_trait! {
    pub trait Chain {
        /// Sends signed transaction, returning its hash.
        # [rpc(name = "chain_sendSignedTransaction")]
        fn send_signed_transaction(&self, Bytes) -> Result<H256>;
    }
}
