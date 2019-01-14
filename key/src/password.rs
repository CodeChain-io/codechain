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

use std::str::FromStr;
use std::{fmt, ptr};

use never::Never;

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Password(String);

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Password(******)")
    }
}

impl Password {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

// Custom drop impl to zero out memory.
impl Drop for Password {
    fn drop(&mut self) {
        unsafe {
            for byte_ref in self.0.as_mut_vec() {
                ptr::write_volatile(byte_ref, 0)
            }
        }
    }
}

impl FromStr for Password {
    type Err = Never;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        Ok(Password(s.to_string()))
    }
}

impl From<String> for Password {
    fn from(s: String) -> Self {
        s.parse().unwrap()
    }
}

impl<'a> From<&'a str> for Password {
    fn from(s: &'a str) -> Self {
        s.parse().unwrap()
    }
}
