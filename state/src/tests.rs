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
    use std::sync::Arc;

    use cvm::ChainTimeInfo;

    use kvdb::KeyValueDB;
    use kvdb_memorydb;
    use primitives::H256;

    use crate::impls::TopLevelState;
    use crate::StateDB;

    pub struct TestChainTimeInfoClient {}

    impl ChainTimeInfo for TestChainTimeInfoClient {
        fn best_block_number(&self) -> u64 {
            0
        }

        fn best_block_timestamp(&self) -> u64 {
            0
        }

        fn transaction_block_age(&self, _: &H256) -> Option<u64> {
            Some(0)
        }

        fn transaction_time_age(&self, _: &H256) -> Option<u64> {
            Some(0)
        }
    }

    pub fn get_memory_db() -> Arc<KeyValueDB> {
        Arc::new(kvdb_memorydb::create(1))
    }

    pub fn get_temp_state_db() -> StateDB {
        StateDB::new_with_memorydb(Vec::new())
    }

    pub fn get_temp_state() -> TopLevelState {
        let state_db = get_temp_state_db();
        TopLevelState::new_for_testing(state_db)
    }

    pub fn get_test_client() -> TestChainTimeInfoClient {
        TestChainTimeInfoClient {}
    }
}
