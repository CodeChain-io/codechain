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

use ccore::AccountProvider;
use ctypes::Address;
use jsonrpc_core::Result;

use super::super::errors::account_provider;
use super::super::traits::Account;

pub struct AccountClient {
    account_provider: Arc<AccountProvider>,
}

impl AccountClient {
    pub fn new(ap: &Arc<AccountProvider>) -> Self {
        AccountClient {
            account_provider: ap.clone(),
        }
    }
}

impl Account for AccountClient {
    fn get_account_list(&self) -> Result<Vec<Address>> {
        self.account_provider.get_list().map_err(account_provider)
    }
}
