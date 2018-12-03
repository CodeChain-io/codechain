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

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use cio::IoChannel;
use kvdb::DBTransaction;
use parking_lot::Mutex;
use primitives::H256;
use rlp::Encodable;

use super::BlockInfo;
use super::{Client, ClientConfig};
use crate::block::{enact, IsBlock, LockedBlock};
use crate::blockchain::{BlockChain, BodyProvider, HeaderProvider, ImportRoute};
use crate::consensus::epoch::Transition as EpochTransition;
use crate::consensus::CodeChainEngine;
use crate::error::Error;
use crate::header::Header;
use crate::miner::{Miner, MinerService};
use crate::service::ClientIoMessage;
use crate::types::BlockId;
use crate::verification::queue::{BlockQueue, HeaderQueue};
use crate::verification::{self, PreverifiedBlock, Verifier};
use crate::views::{BlockView, HeaderView};

pub struct Importer {
    /// Lock used during block import
    pub import_lock: Mutex<()>, // FIXME Maybe wrap the whole `Importer` instead?

    /// Used to verify blocks
    pub verifier: Box<Verifier<Client>>,

    /// Queue containing pending blocks
    pub block_queue: BlockQueue,

    /// Queue containing pending headers
    pub header_queue: HeaderQueue,

    /// Handles block sealing
    pub miner: Arc<Miner>,

    /// CodeChain engine to be used during import
    pub engine: Arc<CodeChainEngine>,
}

impl Importer {
    pub fn try_new(
        config: &ClientConfig,
        engine: Arc<CodeChainEngine>,
        message_channel: IoChannel<ClientIoMessage>,
        miner: Arc<Miner>,
    ) -> Result<Importer, Error> {
        let block_queue = BlockQueue::new(
            &config.queue,
            engine.clone(),
            message_channel.clone(),
            config.verifier_type.verifying_seal(),
        );

        let header_queue =
            HeaderQueue::new(&config.queue, engine.clone(), message_channel, config.verifier_type.verifying_seal());

        Ok(Importer {
            import_lock: Mutex::new(()),
            verifier: verification::new(config.verifier_type),
            block_queue,
            header_queue,
            miner,
            engine,
        })
    }

    /// This is triggered by a message coming from a block queue when the block is ready for insertion
    pub fn import_verified_blocks(&self, client: &Client) -> usize {
        let max_blocks_to_import = 4;
        let (imported_blocks, import_results, invalid_blocks, imported, duration, is_empty) = {
            let mut imported_blocks = Vec::with_capacity(max_blocks_to_import);
            let mut invalid_blocks = HashSet::new();
            let mut import_results = Vec::with_capacity(max_blocks_to_import);

            let _import_lock = self.import_lock.lock();
            let blocks = self.block_queue.drain(max_blocks_to_import);
            if blocks.is_empty() {
                return 0
            }
            let start = Instant::now();

            for block in blocks {
                let header = &block.header;
                ctrace!(CLIENT, "Importing block {}", header.number());
                let is_invalid = invalid_blocks.contains(header.parent_hash());
                if is_invalid {
                    invalid_blocks.insert(header.hash());
                    continue
                }
                if let Ok(closed_block) = self.check_and_close_block(&block, client) {
                    if self.engine.is_proposal(&block.header) {
                        self.engine.on_verified_proposal(&header);
                        self.block_queue.mark_as_good(&[header.hash()]);
                    } else {
                        imported_blocks.push(header.hash());

                        let route = self.commit_block(&closed_block, &header, &block.bytes, client);
                        import_results.push(route);
                    }
                } else {
                    invalid_blocks.insert(header.hash());
                }
            }

            let imported = imported_blocks.len();
            let invalid_blocks = invalid_blocks.into_iter().collect::<Vec<H256>>();

            if !invalid_blocks.is_empty() {
                self.block_queue.mark_as_bad(&invalid_blocks);
            }
            let is_empty = self.block_queue.mark_as_good(&imported_blocks);
            let duration_ns = {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1_000_000_000 + u64::from(elapsed.subsec_nanos())
            };
            (imported_blocks, import_results, invalid_blocks, imported, duration_ns, is_empty)
        };

        {
            if !imported_blocks.is_empty() && is_empty {
                let (enacted, retracted) = self.calculate_enacted_retracted(&import_results);

                if is_empty {
                    self.miner.chain_new_blocks(client, &imported_blocks, &invalid_blocks, &enacted, &retracted);
                }

                client.notify(|notify| {
                    notify.new_blocks(
                        imported_blocks.clone(),
                        invalid_blocks.clone(),
                        enacted.clone(),
                        retracted.clone(),
                        Vec::new(),
                        duration,
                    );
                });
            }
        }

        client.db().flush().expect("DB flush failed.");
        imported
    }

    pub fn calculate_enacted_retracted(&self, import_results: &[ImportRoute]) -> (Vec<H256>, Vec<H256>) {
        fn map_to_vec(map: Vec<(H256, bool)>) -> Vec<H256> {
            map.into_iter().map(|(k, _v)| k).collect()
        }

        // In ImportRoute we get all the blocks that have been enacted and retracted by single insert.
        // Because we are doing multiple inserts some of the blocks that were enacted in import `k`
        // could be retracted in import `k+1`. This is why to understand if after all inserts
        // the block is enacted or retracted we iterate over all routes and at the end final state
        // will be in the hashmap
        let map = import_results.iter().fold(HashMap::new(), |mut map, route| {
            for hash in &route.enacted {
                map.insert(*hash, true);
            }
            for hash in &route.retracted {
                map.insert(*hash, false);
            }
            map
        });

        // Split to enacted retracted (using hashmap value)
        let (enacted, retracted) = map.into_iter().partition(|&(_k, v)| v);
        // And convert tuples to keys
        (map_to_vec(enacted), map_to_vec(retracted))
    }

    // NOTE: the header of the block passed here is not necessarily sealed, as
    // it is for reconstructing the state transition.
    //
    // The header passed is from the original block data and is sealed.
    pub fn commit_block<B>(&self, block: &B, header: &Header, block_data: &[u8], client: &Client) -> ImportRoute
    where
        B: IsBlock, {
        let hash = header.hash();
        let number = header.number();

        let chain = client.block_chain();

        // Commit results
        let invoices = block.invoices().to_owned();

        assert_eq!(hash, BlockView::new(block_data).header_view().hash());

        let mut batch = DBTransaction::new();

        // check epoch end signal
        self.check_epoch_end_signal(block.header(), &chain, &mut batch);

        block.state().journal_under(&mut batch, number).expect("DB commit failed");
        let route = chain.insert_block(&mut batch, block_data, invoices.clone(), self.engine.borrow());

        // Final commit to the DB
        client.db().write_buffered(batch);
        chain.commit();

        self.check_epoch_end(block.header(), &chain, client);

        if hash == chain.best_block_hash() {
            let mut state_db = client.state_db().write();
            let state = block.state();
            state_db.override_state(&state);
        }

        route
    }

    // check for ending of epoch and write transition if it occurs.
    fn check_epoch_end(&self, header: &Header, chain: &BlockChain, client: &Client) {
        let is_epoch_end = self.engine.is_epoch_end(
            header,
            &(|hash| chain.block_header(&hash)),
            &(|hash| chain.get_pending_transition(hash)), // TODO: limit to current epoch.
        );

        if let Some(proof) = is_epoch_end {
            cdebug!(CLIENT, "Epoch transition at block {}", header.hash());

            let mut batch = DBTransaction::new();
            chain.insert_epoch_transition(
                &mut batch,
                header.number(),
                EpochTransition {
                    block_hash: header.hash(),
                    block_number: header.number(),
                    proof,
                },
            );

            // always write the batch directly since epoch transition proofs are
            // fetched from a DB iterator and DB iterators are only available on
            // flushed data.
            client.db().write(batch).expect("DB flush failed");
        }
    }

    // check for epoch end signal and write pending transition if it occurs.
    // state for the given block must be available.
    fn check_epoch_end_signal(&self, header: &Header, chain: &BlockChain, batch: &mut DBTransaction) {
        use crate::consensus::EpochChange;
        let hash = header.hash();

        match self.engine.signals_epoch_end(header) {
            EpochChange::Yes(proof) => {
                use crate::consensus::epoch::PendingTransition;
                use crate::consensus::Proof;

                let Proof::Known(proof) = proof;
                cdebug!(CLIENT, "Block {} signals epoch end.", hash);

                let pending = PendingTransition {
                    proof,
                };
                chain.insert_pending_transition(batch, hash, &pending);
            }
            EpochChange::No => {}
            EpochChange::Unsure => {
                cwarn!(CLIENT, "Detected invalid engine implementation.");
                cwarn!(CLIENT, "Engine claims to require more block data, but everything provided.");
            }
        }
    }

    fn check_and_close_block(&self, block: &PreverifiedBlock, client: &Client) -> Result<LockedBlock, ()> {
        let engine = &*self.engine;
        let header = &block.header;

        let chain = client.block_chain();

        // Check if parent is in chain
        let parent = chain.block_header(header.parent_hash()).ok_or_else(|| {
            cwarn!(
                CLIENT,
                "Block import failed for #{} ({}): Parent not found ({}) ",
                header.number(),
                header.hash(),
                header.parent_hash()
            );
        })?;

        chain.block_body(header.parent_hash()).ok_or_else(|| {
            cerror!(
                CLIENT,
                "Block import failed for #{} ({}): Parent block not found ({}) ",
                header.number(),
                header.hash(),
                parent.hash()
            );
        })?;

        // Verify Block Family
        self.verifier
            .verify_block_family(
                &block.bytes,
                header,
                &parent,
                engine,
                Some(verification::FullFamilyParams {
                    block_bytes: &block.bytes,
                    parcels: &block.parcels,
                    block_provider: &*chain,
                    client,
                }),
            )
            .map_err(|e| {
                cwarn!(
                    CLIENT,
                    "Stage 3 block verification failed for #{} ({})\nError: {:?}",
                    header.number(),
                    header.hash(),
                    e
                );
            })?;

        self.verifier.verify_block_external(header, engine).map_err(|e| {
            cwarn!(
                CLIENT,
                "Stage 4 block verification failed for #{} ({})\nError: {:?}",
                header.number(),
                header.hash(),
                e
            );
        })?;


        // Enact Verified Block
        let db = client.state_db().read().clone(&parent.state_root());

        let is_epoch_begin = chain.epoch_transition(parent.number(), *header.parent_hash()).is_some();
        let enact_result = enact(&block.header, &block.parcels, engine, client, db, &parent, is_epoch_begin);
        let locked_block = enact_result.map_err(|e| {
            cwarn!(CLIENT, "Block import failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
        })?;

        // Final Verification
        self.verifier.verify_block_final(header, locked_block.block().header()).map_err(|e| {
            cwarn!(
                CLIENT,
                "Stage 5 block verification failed for #{} ({})\nError: {:?}",
                header.number(),
                header.hash(),
                e
            );
        })?;

        Ok(locked_block)
    }

    /// This is triggered by a message coming from a header queue when the header is ready for insertion
    pub fn import_verified_headers(&self, client: &Client) -> usize {
        let max_headers_to_import = 256;

        let _lock = self.import_lock.lock();
        let prev_highest_header_hash = client.block_chain().highest_header().hash();

        let mut bad = HashSet::new();
        let mut imported = Vec::new();
        let mut routes = Vec::new();
        for header in self.header_queue.drain(max_headers_to_import) {
            let hash = header.hash();
            ctrace!(CLIENT, "Importing header {}", header.number());

            if bad.contains(&hash) || bad.contains(header.parent_hash()) {
                ctrace!(CLIENT, "Bad header detected : {}", hash);
                bad.insert(hash);
                continue
            }

            let parent_header = client
                .block_header(&BlockId::Hash(*header.parent_hash()))
                .expect("Parent of importing header must exist")
                .decode();
            if self.check_header(&header, &parent_header) {
                if self.engine.is_proposal(&header) {
                    self.header_queue.mark_as_good(&[hash]);
                } else {
                    imported.push(hash);
                    routes.push(self.commit_header(&header, client));
                }
            } else {
                bad.insert(hash);
            }
        }

        self.header_queue.mark_as_bad(&bad.drain().collect::<Vec<_>>());
        let (enacted, retracted) = self.calculate_enacted_retracted(&routes);

        let new_highest_header_hash = client.block_chain().highest_header().hash();
        let highest_header_changed = if prev_highest_header_hash != new_highest_header_hash {
            Some(new_highest_header_hash)
        } else {
            None
        };

        client.notify(|notify| {
            notify.new_headers(
                imported.clone(),
                bad.iter().cloned().collect(),
                enacted.clone(),
                retracted.clone(),
                Vec::new(),
                0,
                highest_header_changed,
            );
        });

        client.db().flush().expect("DB flush failed.");

        imported.len()
    }

    fn check_header(&self, header: &Header, parent: &Header) -> bool {
        // FIXME: self.verifier.verify_block_family
        if let Err(e) = self.engine.verify_block_family(&header, &parent) {
            cwarn!(
                CLIENT,
                "Stage 3 block verification failed for #{} ({})\nError: {:?}",
                header.number(),
                header.hash(),
                e
            );
            return false
        };

        // "external" verification.
        if let Err(e) = self.engine.verify_block_external(&header) {
            cwarn!(
                CLIENT,
                "Stage 4 block verification failed for #{} ({})\nError: {:?}",
                header.number(),
                header.hash(),
                e
            );
            return false
        };

        true
    }

    fn commit_header(&self, header: &Header, client: &Client) -> ImportRoute {
        let chain = client.block_chain();

        let mut batch = DBTransaction::new();
        // FIXME: Check if this line is still necessary.
        // self.check_epoch_end_signal(header, &chain, &mut batch);
        let route = chain.insert_header(&mut batch, &HeaderView::new(&header.rlp_bytes()), self.engine.borrow());
        client.db().write_buffered(batch);
        chain.commit();

        // FIXME: Check if this line is still necessary.
        // self.check_epoch_end(&header, &chain, client);

        route
    }
}
