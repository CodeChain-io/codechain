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

import { expect } from "chai";
import { H160, H256, U256 } from "codechain-primitives/lib";
import { TestHelper } from "codechain-test-helper";
import { Header } from "codechain-test-helper/lib/cHeader";
import "mocha";
import CodeChain from "../helper/spawn";

describe("Test onChain block communication", async function() {
    let nodeA: CodeChain;
    let TH: TestHelper;
    let soloGenesisBlock: Header;
    let soloBlock1: Header;
    let soloBlock2: Header;

    let VALID_PARENT = new H256(
        "ff8324bd3b0232e4fd1799496ae422ee0896cc7a8a64a2885052e320b4ba9535"
    );
    let VALID_AUTHOR = new H160("7777777777777777777777777777777777777777");
    let VALID_TRANSACTIONS_ROOT = new H256(
        "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0"
    );
    let VALID_STATEROOT = new H256(
        "2f6b19afc38f6f1464af20dde08d8bebd6a6aec0a95aaf7ef2fb729c3b88dc5b"
    );
    let VALID_INVOICEROOT = new H256(
        "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0"
    );

    const BASE = 350;

    before(async function() {
        const node = new CodeChain({
            argv: ["--force-sealing"],
            base: BASE
        });
        await node.start();

        const sdk = node.sdk;

        await sdk.rpc.devel.startSealing();
        await sdk.rpc.devel.startSealing();

        const genesisBlock = await sdk.rpc.chain.getBlock(0);
        if (genesisBlock == null) {
            throw Error("Cannot get the genesis block");
        }
        const block1 = await sdk.rpc.chain.getBlock(1);
        if (block1 == null) {
            throw Error("Cannot get the first block");
        }
        const block2 = await sdk.rpc.chain.getBlock(2);
        if (block2 == null) {
            throw Error("Cannot get the second block");
        }

        await node.clean();
        soloGenesisBlock = new Header(
            genesisBlock.parentHash,
            new U256(genesisBlock.timestamp),
            new U256(genesisBlock.number),
            genesisBlock.author.accountId,
            Buffer.from(genesisBlock.extraData),
            genesisBlock.transactionsRoot,
            genesisBlock.stateRoot,
            genesisBlock.invoicesRoot,
            genesisBlock.score,
            genesisBlock.seal
        );
        soloBlock1 = new Header(
            soloGenesisBlock.hashing(),
            new U256(block1.timestamp),
            new U256(block1.number),
            block1.author.accountId,
            Buffer.from(block1.extraData),
            block1.transactionsRoot,
            block1.stateRoot,
            block1.invoicesRoot,
            new U256(2222222222222),
            block1.seal
        );
        soloBlock2 = new Header(
            soloBlock1.hashing(),
            new U256(block2.timestamp),
            new U256(block2.number),
            block2.author.accountId,
            Buffer.from(block2.extraData),
            block2.transactionsRoot,
            block2.stateRoot,
            block2.invoicesRoot,
            new U256(33333333333333),
            block2.seal
        );

        VALID_PARENT = block1.parentHash;
        VALID_AUTHOR = block1.author.accountId;
        VALID_TRANSACTIONS_ROOT = block1.transactionsRoot;
        VALID_STATEROOT = block1.stateRoot;
        VALID_INVOICEROOT = block1.invoicesRoot;

        nodeA = new CodeChain({ base: BASE });
        await nodeA.start();
        TH = new TestHelper("0.0.0.0", nodeA.port);
        await TH.establish();
    });

    it("OnChain valid block propagation test", async function() {
        // TH.setLog();
        const sdk = nodeA.sdk;

        // Genesis block
        const header = soloGenesisBlock;

        // Block 1
        const header1 = soloBlock1;

        // Block 2
        const header2 = soloBlock2;

        await TH.sendEncodedBlock(
            [
                header.toEncodeObject(),
                header1.toEncodeObject(),
                header2.toEncodeObject()
            ],
            [[], []],
            header2.hashing(),
            header2.getScore()
        );

        await TH.waitStatusMessage();

        const block1 = await sdk.rpc.chain.getBlock(1);
        const block2 = await sdk.rpc.chain.getBlock(2);

        expect(block1).not.to.be.null;
        expect(block2).not.to.be.null;
    }).timeout(10_000);

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodeA.testFailed(this.currentTest!.fullTitle());
        }
        await TH.end();
        await nodeA.clean();
    });
});
