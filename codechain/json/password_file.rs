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

use serde_json;

use super::password_entry::PasswordEntry;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PasswordFile(Vec<PasswordEntry>);

impl PasswordFile {
    pub fn load<R>(reader: R) -> Result<Self, serde_json::Error>
    where
        R: Read, {
        serde_json::from_reader(reader)
    }

    pub fn entries(&self) -> &[PasswordEntry] {
        self.0.as_slice()
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
                "address": "tccq9c4d7e9rpqsp833qk96n9q6m5jcms0uzs7lc5ah",
                "password": "mypass1"
            },
            {
                "address": "tccq8eemdzrfyrc3fjanwpk4ttyd8ykt67yvvwz64au",
                "password": "mypass2"
            }
		]"#;

        let expected = PasswordFile(vec![
            PasswordEntry {
                address: "tccq9c4d7e9rpqsp833qk96n9q6m5jcms0uzs7lc5ah".into(),
                password: "mypass1".into(),
            },
            PasswordEntry {
                address: "tccq8eemdzrfyrc3fjanwpk4ttyd8ykt67yvvwz64au".into(),
                password: "mypass2".into(),
            },
        ]);

        let pf: PasswordFile = serde_json::from_str(json).unwrap();
        assert_eq!(pf, expected);
    }
}
