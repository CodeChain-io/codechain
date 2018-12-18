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

extern crate limited_table;

use std::convert::Into;

use limited_table::{Key, LimitedTable};

pub type Token = Key;

pub struct TokenGenerator {
    limited: LimitedTable<()>,
}

impl TokenGenerator {
    pub fn new<T: Into<Token>>(begin: T, limit: usize) -> Self {
        Self {
            limited: LimitedTable::new(begin.into(), limit),
        }
    }

    pub fn is_assigned(&self, token: Token) -> bool {
        self.limited.contains(token)
    }

    pub fn gen(&mut self) -> Option<Token> {
        self.limited.insert(())
    }

    pub fn restore(&mut self, token: Token) -> bool {
        self.limited.remove(token).is_some()
    }
}
