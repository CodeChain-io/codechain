// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use super::super::json;

#[derive(Debug, PartialEq, Clone)]
pub struct Aes128Ctr {
    pub iv: [u8; 16],
}

#[derive(Debug, PartialEq, Clone)]
pub enum Cipher {
    Aes128Ctr(Aes128Ctr),
}

impl From<json::Aes128Ctr> for Aes128Ctr {
    fn from(json: json::Aes128Ctr) -> Self {
        Aes128Ctr {
            iv: json.iv.into(),
        }
    }
}

impl From<Aes128Ctr> for json::Aes128Ctr {
    fn from(aes: Aes128Ctr) -> Self {
        Self {
            iv: From::from(aes.iv),
        }
    }
}

impl From<json::Cipher> for Cipher {
    fn from(json: json::Cipher) -> Self {
        match json {
            json::Cipher::Aes128Ctr(params) => Cipher::Aes128Ctr(From::from(params)),
        }
    }
}

impl From<Cipher> for json::Cipher {
    fn from(cipher: Cipher) -> Self {
        match cipher {
            Cipher::Aes128Ctr(params) => json::Cipher::Aes128Ctr(params.into()),
        }
    }
}
