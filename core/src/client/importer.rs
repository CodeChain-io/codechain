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

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use cio::IoChannel;
use kvdb::DBTransaction;
use parking_lot::{Mutex, MutexGuard};
use primitives::H256;
use rlp::Encodable;

use super::{BlockChainTrait, Client, ClientConfig};
use crate::block::{enact, IsBlock, LockedBlock};
use crate::blockchain::{BodyProvider, HeaderProvider, ImportRoute};
use crate::consensus::CodeChainEngine;
use crate::encoded;
use crate::error::Error;
use crate::header::Header;
use crate::miner::{Miner, MinerService};
use crate::service::ClientIoMessage;
use crate::types::BlockId;
use crate::verification::queue::{BlockQueue, HeaderQueue};
use crate::verification::{self, PreverifiedBlock, Verifier};
use crate::views::{BlockView, HeaderView};
use blockchain::BlockChain;

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
        let (imported_blocks, import_results, invalid_blocks, imported, duration, is_empty) = {
            const MAX_BLOCKS_TO_IMPORT: usize = 1_000;
            let mut imported_blocks = Vec::with_capacity(MAX_BLOCKS_TO_IMPORT);
            let mut invalid_blocks = HashSet::new();
            let mut import_results = Vec::with_capacity(MAX_BLOCKS_TO_IMPORT);

            let import_lock = self.import_lock.lock();
            let blocks = self.block_queue.drain(MAX_BLOCKS_TO_IMPORT);
            if blocks.is_empty() {
                return 0
            }

            {
                let headers: Vec<&Header> = blocks.iter().map(|block| &block.header).collect();
                self.import_headers(headers, client, &import_lock);
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
                        self.engine.on_verified_proposal(encoded::Block::new(block.bytes.clone()))
                    }

                    imported_blocks.push(header.hash());
                    let route = self.commit_block(&closed_block, &header, &block.bytes, client);
                    import_results.push(route);
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
                self.miner.chain_new_blocks(client, &imported_blocks, &invalid_blocks, &enacted, &retracted);
                client.new_blocks(&imported_blocks, &invalid_blocks, &enacted, &retracted, &[], duration);
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

        block.state().journal_under(&mut batch, number).expect("DB commit failed");
        let route = chain.insert_block(&mut batch, block_data, invoices.clone(), self.engine.borrow());

        // Final commit to the DB
        client.db().write_buffered(batch);
        chain.commit();

        self.check_term_end(block.header(), &chain, client);

        if hash == chain.best_block_hash() {
            let mut state_db = client.state_db().write();
            let state = block.state();
            state_db.override_state(&state);
        }

        route
    }

    fn check_term_end(&self, header: &Header, chain: &BlockChain, client: &Client) {
        let last_term_end = chain.last_term_end();
        if let Some(term_id) = self.engine.is_term_end(header, last_term_end) {
            cdebug!(CLIENT, "Term end at block {}", header.hash());

            let mut batch = DBTransaction::new();
            chain.update_last_term_end(&mut batch, term_id, header.number());

            // always write the batch directly since term end are
            // fetched from a DB iterator and DB iterators are only available on
            // flushed data.
            client.db().write(batch).expect("DB flush failed");
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
                    transactions: &block.transactions,
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

        let enact_result = enact(&block.header, &block.transactions, engine, client, db, &parent);
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
        const MAX_HEADERS_TO_IMPORT: usize = 10_000;
        let lock = self.import_lock.lock();
        let headers = self.header_queue.drain(MAX_HEADERS_TO_IMPORT);
        self.import_headers(&headers, client, &lock)
    }

    pub fn import_headers<'a>(
        &'a self,
        headers: impl IntoIterator<Item = &'a Header>,
        client: &Client,
        _importer_lock: &MutexGuard<()>,
    ) -> usize {
        let prev_best_proposal_header_hash = client.block_chain().best_proposal_header().hash();

        let mut bad = HashSet::new();
        let mut imported = Vec::new();
        let mut routes = Vec::new();

        for header in headers {
            let hash = header.hash();
            ctrace!(CLIENT, "Importing header {}-{:?}", header.number(), hash);

            if bad.contains(&hash) || bad.contains(header.parent_hash()) {
                cinfo!(CLIENT, "Bad header detected : {}", hash);
                bad.insert(hash);
                continue
            }

            let parent_header = client
                .block_header(&BlockId::Hash(*header.parent_hash()))
                .unwrap_or_else(|| panic!("Parent of importing header must exist {:?}", header.parent_hash()))
                .decode();
            if client.block_header(&BlockId::Hash(hash)).is_some() {
                // Do nothing if the header is already imported
            } else if self.check_header(&header, &parent_header) {
                imported.push(hash);
                routes.push(self.commit_header(&header, client));
            } else {
                bad.insert(hash);
            }
        }

        self.header_queue.mark_as_bad(&bad.drain().collect::<Vec<_>>());
        let (enacted, retracted) = self.calculate_enacted_retracted(&routes);

        let new_best_proposal_header_hash = client.block_chain().best_proposal_header().hash();
        let best_proposal_header_changed = if prev_best_proposal_header_hash != new_best_proposal_header_hash {
            Some(new_best_proposal_header_hash)
        } else {
            None
        };

        client.new_headers(
            &imported,
            &bad.iter().cloned().collect::<Vec<_>>(),
            &enacted,
            &retracted,
            &[],
            0,
            best_proposal_header_changed,
        );

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
        let route = chain.insert_header(&mut batch, &HeaderView::new(&header.rlp_bytes()), self.engine.borrow());
        client.db().write_buffered(batch);
        chain.commit();

        // FIXME: Check if this line is still necessary.
        // self.check_term_end(&header, &chain, client);

        route
    }
}
