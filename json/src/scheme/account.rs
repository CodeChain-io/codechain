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

use crate::uint::Uint;

/// Scheme account.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Account {
    /// Balance.
    pub balance: Option<Uint>,
    /// Seq.
    pub seq: Option<Uint>,
}

impl Account {
    /// Returns true if account does not have seq and balance
    pub fn is_empty(&self) -> bool {
        self.balance.is_none() && self.seq.is_none()
    }
}

#[cfg(test)]
mod tests {
    use primitives::U256;
    use serde_json;

    use super::Account;
    use crate::uint::Uint;

    #[test]
    fn account_deserialization() {
        let s = r#"{
            "balance": "1",
            "seq": "0"
        }"#;
        let deserialized: Account = serde_json::from_str(s).unwrap();
        assert!(!deserialized.is_empty());
        assert_eq!(deserialized.balance.unwrap(), Uint(U256::from(1)));
        assert_eq!(deserialized.seq.unwrap(), Uint(U256::from(0)));
    }
}
