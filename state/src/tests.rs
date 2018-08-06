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

pub mod helpers {
    use super::super::impls::TopLevelState;
    use super::super::StateDB;

    pub fn get_temp_state_db() -> StateDB {
        StateDB::new_with_memorydb(5 * 1024 * 1024, Vec::new())
    }

    pub fn get_temp_state() -> TopLevelState {
        let journal_db = get_temp_state_db();
        TopLevelState::new(journal_db)
    }
}
