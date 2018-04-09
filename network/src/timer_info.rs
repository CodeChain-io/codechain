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

use std::result;

use ctable::Table;

use super::limited_table::{Key as TimerToken, LimitedTable};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    DuplicatedTimerId,
    NoSpace,
}

pub type Result<T> = result::Result<T, Error>;

type TimerId = usize;

#[derive(Clone)]
pub struct TimerItem {
    pub name: String,
    pub timer_id: TimerId,
    pub once: bool,
}

pub struct TimerInfo {
    tokens: LimitedTable<TimerItem>,
    reversed: Table<String, TimerId, TimerToken>,
}

impl TimerInfo {
    pub fn new(begin: TimerToken, limit: usize) -> Self {
        Self {
            tokens: LimitedTable::new(begin, limit),
            reversed: Table::new(),
        }
    }

    pub fn insert(&mut self, name: String, timer_id: TimerId, once: bool) -> Result<TimerToken> {
        if self.reversed.get(&name, &timer_id).is_some() {
            return Err(Error::DuplicatedTimerId)
        }
        self.tokens
            .insert(TimerItem {
                name: name.clone(),
                timer_id,
                once,
            })
            .map(|token| {
                self.reversed.insert(name.clone(), timer_id, token);
                token
            })
            .ok_or(Error::NoSpace)
    }

    pub fn get_info(&self, token: TimerToken) -> Option<TimerItem> {
        self.tokens.get(token).map(|info| info.clone())
    }

    pub fn remove_by_token(&mut self, token: TimerToken) {
        if let Some(TimerItem {
            name,
            timer_id,
            ..
        }) = self.tokens.remove(token)
        {
            self.reversed.remove(&name, &timer_id);
        }
    }

    pub fn remove_by_info(&mut self, name: String, timer_id: TimerId) -> Option<TimerToken> {
        self.reversed.remove(&name, &timer_id).map(|token| {
            self.tokens.remove(token);
            token
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use super::TimerInfo;

    #[test]
    fn add() {
        let mut timer = TimerInfo::new(0, 4);
        assert_eq!(Ok(0), timer.insert("a".to_string(), 1, false));
        assert_eq!(Ok(1), timer.insert("a".to_string(), 2, false));
    }

    #[test]
    fn timer_id_cannot_be_duplicated_if_name_is_same() {
        let mut timer = TimerInfo::new(0, 4);
        assert_eq!(Ok(0), timer.insert("a".to_string(), 1, false));
        assert_eq!(Err(Error::DuplicatedTimerId), timer.insert("a".to_string(), 1, true));
    }

    #[test]
    fn timer_id_can_be_duplicated_if_name_is_different() {
        let mut timer = TimerInfo::new(0, 4);
        assert_eq!(Ok(0), timer.insert("a".to_string(), 1, false));
        assert_eq!(Ok(1), timer.insert("b".to_string(), 1, false));
    }
}
