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

use std::io::Read;

use ckey::{Address, Password};
use serde_json;

use super::password_entry::PasswordEntry;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PasswordFile(Vec<PasswordEntry>);

impl PasswordFile {
    pub fn load<R>(reader: R) -> Result<Self, serde_json::Error>
    where
        R: Read, {
        serde_json::from_reader(reader)
    }

    pub fn password(&self, address: &Address) -> Option<Password> {
        for entry in &self.0 {
            if address == &entry.address.address {
                return Some(entry.password.clone())
            }
        }
        None
    }
}

impl Default for PasswordFile {
    fn default() -> Self {
        PasswordFile(vec![])
    }
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::super::password_entry::PasswordEntry;
    use super::PasswordFile;

    #[test]
    fn password_file() {
        let json = r#"
		[
            {
                "address": "cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5",
                "password": "mypass1"
            },
            {
                "address": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7",
                "password": "mypass2"
            }
		]"#;

        let expected = PasswordFile(vec![
            PasswordEntry {
                address: "cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5".into(),
                password: "mypass1".into(),
            },
            PasswordEntry {
                address: "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7".into(),
                password: "mypass2".into(),
            },
        ]);

        let pf: PasswordFile = serde_json::from_str(json).unwrap();
        assert_eq!(pf, expected);
    }
}
