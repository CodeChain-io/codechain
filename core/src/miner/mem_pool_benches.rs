// Copyright 2019 Kodebox, Inc.
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

extern crate test;

use std::sync::Arc;

use ckey::{Generator, KeyPair, Public, Random};
use ctypes::transaction::{Action, Transaction};
use rand::prelude::SliceRandom;
use rand::thread_rng;

use self::test::{black_box, Bencher};
use super::mem_pool::MemPool;
use super::mem_pool_types::{AccountDetails, MemPoolInput, PoolingInstant, TxOrigin, TxTimelock};
use crate::transaction::SignedTransaction;

const NUM_TXS: usize = 5000;

fn create_input(keypair: &KeyPair, seq: u64, block: Option<PoolingInstant>, timestamp: Option<u64>) -> MemPoolInput {
    let tx = Transaction {
        seq,
        fee: 100,
        network_id: "tc".into(),
        action: Action::Pay {
            receiver: 0.into(),
            quantity: 100,
        },
    };
    let timelock = TxTimelock {
        block,
        timestamp,
    };
    let signed = SignedTransaction::new_with_sign(tx, keypair.private());

    MemPoolInput::new(signed, TxOrigin::Local, timelock)
}

#[bench]
pub fn add_in_ascending_order(bencher: &mut Bencher) {
    let fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, i as u64, None, None));
    }

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &fetch_account);
        }
        black_box(mem_pool);
    });
}

#[bench]
pub fn add_in_descending_order(bencher: &mut Bencher) {
    let fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, (NUM_TXS - i - 1) as u64, None, None));
    }

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &fetch_account);
        }
        black_box(mem_pool);
    });
}

#[bench]
pub fn add_randomly(bencher: &mut Bencher) {
    let fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, i as u64, None, None));
    }

    inputs.shuffle(&mut thread_rng());

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &fetch_account);
        }
        black_box(mem_pool);
    });
}

#[bench]
pub fn add_then_remove_in_ascending_order(bencher: &mut Bencher) {
    let fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let fetch_seq = |_p: &Public| -> u64 { 0 };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, i as u64, None, None));
    }

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &fetch_account);
        }
        for input in inputs {
            mem_pool.remove(&vec![input.transaction.hash()], &fetch_seq, current_time, current_timestamp);
        }
        black_box(mem_pool);
    });
}

#[bench]
pub fn add_then_remove_in_descending_order(bencher: &mut Bencher) {
    let fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let fetch_seq = |_p: &Public| -> u64 { 0 };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, i as u64, None, None));
    }

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &fetch_account);
        }
        for input in inputs.iter().rev() {
            mem_pool.remove(&vec![input.transaction.hash()], &fetch_seq, current_time, current_timestamp);
        }
        black_box(mem_pool);
    });
}

#[bench]
pub fn add_then_remove_old(bencher: &mut Bencher) {
    let old_fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: 0,
            balance: u64::max_value(),
        }
    };

    let new_fetch_account = |_p: &Public| -> AccountDetails {
        AccountDetails {
            seq: (NUM_TXS / 2) as u64,
            balance: u64::max_value(),
        }
    };

    let keypair = &Random.generate().unwrap();
    let current_time = 100;
    let current_timestamp = 100;

    let mut inputs: Vec<MemPoolInput> = Vec::with_capacity(NUM_TXS);
    for i in 0..NUM_TXS {
        inputs.push(create_input(keypair, i as u64, None, None));
    }

    let inputs = &inputs;
    bencher.iter(|| {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(10000, usize::max_value(), 3, db.clone());
        for input in inputs {
            mem_pool.add(vec![input.clone()], current_time, current_timestamp, &old_fetch_account);
        }
        mem_pool.remove_old(&new_fetch_account, current_time, current_timestamp);
        black_box(mem_pool);
    });
}
