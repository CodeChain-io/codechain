// Copyright 2018-2019 Kodebox, Inc.
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

use kvdb::{DBTransaction, KeyValueDB};
use primitives::H256;

use super::message::ConsensusMessage;
use super::types::{Height, Step, View};
use crate::db;

const BACKUP_KEY: &[u8] = b"tendermint-backup";

pub struct BackupView<'a> {
    pub height: &'a Height,
    pub view: &'a View,
    pub step: &'a Step,
    pub votes: &'a [ConsensusMessage],
    pub last_confirmed_view: &'a View,
}

pub struct BackupData {
    pub height: Height,
    pub view: View,
    pub step: Step,
    pub votes: Vec<ConsensusMessage>,
    pub proposal: Option<H256>,
    pub last_confirmed_view: View,
}

pub fn backup(db: &KeyValueDB, backup_data: BackupView) {
    let BackupView {
        height,
        view,
        step,
        votes,
        last_confirmed_view,
    } = backup_data;
    let mut s = rlp::RlpStream::new();
    s.begin_list(5);
    s.append(height).append(view).append(step).append_list(votes);
    s.append(last_confirmed_view);

    let mut batch = DBTransaction::new();
    batch.put(db::COL_EXTRA, BACKUP_KEY, &s.drain().into_vec());
    db.write(batch).expect("Low level database error. Some issue with disk?");
}

pub fn restore(db: &KeyValueDB) -> Option<BackupData> {
    let value = db.get(db::COL_EXTRA, BACKUP_KEY).expect("Low level database error. Some issue with disk?");
    let (height, view, step, votes, last_confirmed_view) = value.map(|bytes| {
        let bytes = bytes.into_vec();
        let rlp = rlp::Rlp::new(&bytes);
        (rlp.val_at(0), rlp.val_at(1), rlp.val_at(2), rlp.at(3).as_list(), rlp.val_at(4))
    })?;

    let proposal = find_proposal(&votes, height, view);

    Some(BackupData {
        height,
        view,
        step,
        votes,
        proposal,
        last_confirmed_view,
    })
}

fn find_proposal(votes: &[ConsensusMessage], height: Height, view: View) -> Option<H256> {
    votes
        .iter()
        .rev()
        .map(|vote| &vote.on)
        .find(|vote_on| {
            vote_on.step.step == Step::Propose && vote_on.step.view == view && vote_on.step.height == height
        })
        .map(|vote_on| vote_on.block_hash)
        .unwrap_or(None)
}
