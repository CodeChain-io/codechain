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
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

use super::message::{ConsensusMessage, SortitionRound};
use super::types::{Height, Step, View};
use crate::consensus::PriorityInfo;
use crate::db;
use crate::db_version;

const BACKUP_KEY: &[u8] = b"tendermint-backup";
const BACKUP_VERSION: u32 = 1;

pub struct PriorityInfoProjection(pub (SortitionRound, PriorityInfo));

impl PriorityInfoProjection {
    pub fn get_view(&self) -> PriorityInfoProjectionView {
        let (ref sortition_round, ref priority_info) = self.0;
        PriorityInfoProjectionView((sortition_round, priority_info))
    }
}

impl Decodable for PriorityInfoProjection {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 2 {
            Err(DecoderError::RlpIncorrectListLen {
                got: item_count,
                expected: 2,
            })
        } else {
            Ok(Self((rlp.val_at(0)?, rlp.val_at(1)?)))
        }
    }
}

pub struct PriorityInfoProjectionView<'a>(pub (&'a SortitionRound, &'a PriorityInfo));

impl Encodable for PriorityInfoProjectionView<'_> {
    fn rlp_append(&self, s: &mut RlpStream) {
        let (sortition_round, priority_info) = self.0;
        s.begin_list(2).append(sortition_round).append(priority_info);
    }
}

pub struct BackupView<'a> {
    pub height: &'a Height,
    pub view: &'a View,
    pub step: &'a Step,
    pub votes: &'a [ConsensusMessage],
    pub priority_infos: &'a [PriorityInfoProjectionView<'a>],
    pub finalized_view_of_previous_block: &'a View,
    pub finalized_view_of_current_block: &'a Option<View>,
}

#[derive(RlpDecodable)]
pub struct BackupDataV0 {
    pub height: Height,
    pub view: View,
    pub step: Step,
    pub votes: Vec<ConsensusMessage>,
    pub priority_infos: Vec<PriorityInfoProjection>,
    pub last_confirmed_view: View,
}

#[derive(RlpDecodable)]
pub struct BackupDataV1 {
    pub height: Height,
    pub view: View,
    pub step: Step,
    pub votes: Vec<ConsensusMessage>,
    pub priority_infos: Vec<PriorityInfoProjection>,
    pub finalized_view_of_previous_block: View,
    pub finalized_view_of_current_block: Option<View>,
}

pub fn backup(db: &dyn KeyValueDB, backup_data: BackupView) {
    let BackupView {
        height,
        view,
        step,
        votes,
        priority_infos,
        finalized_view_of_previous_block,
        finalized_view_of_current_block,
    } = backup_data;
    let mut s = rlp::RlpStream::new();
    s.begin_list(7);
    s.append(height).append(view).append(step).append_list(votes);
    s.append_list(priority_infos);
    s.append(finalized_view_of_previous_block);
    s.append(finalized_view_of_current_block);

    let mut batch = DBTransaction::new();
    debug_assert!(
        db_version::VERSION_KEY_TENDERMINT_BACKUP.ends_with(BACKUP_KEY),
        "version key should end with the backup key"
    );
    db_version::set_version(&mut batch, db_version::VERSION_KEY_TENDERMINT_BACKUP, BACKUP_VERSION);
    batch.put(db::COL_EXTRA, BACKUP_KEY, &s.drain());
    db.write(batch).expect("Low level database error. Some issue with disk?");
}

pub fn restore(db: &dyn KeyValueDB) -> Option<BackupDataV1> {
    let version = db_version::get_version(db, db_version::VERSION_KEY_TENDERMINT_BACKUP);
    if version < BACKUP_VERSION {
        migrate(db);
    }
    load_with_version::<BackupDataV1>(db)
}

fn migrate(db: &dyn KeyValueDB) {
    let version = db_version::get_version(db, db_version::VERSION_KEY_TENDERMINT_BACKUP);
    assert!(
        version < BACKUP_VERSION,
        "migrate function should be called when the saved version is less than BACKUP_VERSION"
    );

    match version {
        0 => {
            migrate_from_0_to_1(db);
        }
        _ => panic!("Invalid migration version {}", version),
    }
}

fn migrate_from_0_to_1(db: &dyn KeyValueDB) {
    let v0 = if let Some(v0) = load_with_version::<BackupDataV0>(db) {
        v0
    } else {
        return
    };
    let step = v0.step;
    let v1 = BackupDataV1 {
        height: v0.height,
        view: v0.view,
        step: v0.step,
        votes: v0.votes,
        priority_infos: v0.priority_infos,
        // This is not a correct behavior if step == Step::Commit.
        // In Commit state, the Tendermint module overwrote the last_confirmed_view to finalized_view_of_current_block.
        // So we can't restore finalized_view_of_previous block.
        // The code below maintain older code's behavior:
        finalized_view_of_previous_block: v0.last_confirmed_view,
        finalized_view_of_current_block: if step == Step::Commit {
            Some(v0.last_confirmed_view)
        } else {
            None
        },
    };
    backup(db, BackupView {
        height: &v1.height,
        view: &v1.view,
        step: &v1.step,
        votes: &v1.votes,
        priority_infos: &v1.priority_infos.iter().map(|projection| projection.get_view()).collect::<Vec<_>>(),
        finalized_view_of_previous_block: &v1.finalized_view_of_previous_block,
        finalized_view_of_current_block: &v1.finalized_view_of_current_block,
    })
}

fn load_with_version<T: Decodable>(db: &dyn KeyValueDB) -> Option<T> {
    let value = db.get(db::COL_EXTRA, BACKUP_KEY).expect("Low level database error. Some issue with disk?")?;
    let backup: T = rlp::decode(&value).unwrap();

    Some(backup)
}
