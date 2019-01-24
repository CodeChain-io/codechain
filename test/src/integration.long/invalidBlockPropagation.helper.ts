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

import { H160, H256, U256 } from "codechain-primitives/lib";
import { Block } from "codechain-sdk/lib/core/Block";
import { Header } from "codechain-test-helper/lib/cHeader";
import { TestHelper } from "codechain-test-helper/lib/testHelper";
import CodeChain from "../helper/spawn";

import { expect } from "chai";
import "mocha";
import Test = Mocha.Test;
import { PromiseExpect } from "../helper/promise";

async function setup(
    base: number,
    promises: PromiseExpect
): Promise<[Header, Block, Header]> {
    const temporaryNode = new CodeChain({
        argv: ["--force-sealing"],
        base
    });
    await promises.shouldFulfill("tmp.node.start", temporaryNode.start());

    const sdk = temporaryNode.sdk;

    await promises.shouldFulfill(
        "start.sealing.1",
        sdk.rpc.devel.startSealing()
    );
    await promises.shouldFulfill(
        "start.sealing.2",
        sdk.rpc.devel.startSealing()
    );

    const block0 = await promises.shouldFulfill(
        "get.block.0",
        sdk.rpc.chain.getBlock(0)
    );
    if (block0 == null) {
        throw Error("Cannot get the genesis block");
    }
    const block1 = await promises.shouldFulfill(
        "get.block.1",
        sdk.rpc.chain.getBlock(1)
    );
    if (block1 == null) {
        throw Error("Cannot get the first block");
    }
    const block2 = await promises.shouldFulfill(
        "get.block.2",
        sdk.rpc.chain.getBlock(2)
    );
    if (block2 == null) {
        throw Error("Cannot get the second block");
    }

    await promises.shouldFulfill("tmp.node.clean", temporaryNode.clean());
    const header0 = new Header(
        block0.parentHash,
        new U256(block0.timestamp),
        new U256(block0.number),
        block0.author.accountId,
        Buffer.from(block0.extraData),
        block0.transactionsRoot,
        block0.stateRoot,
        block0.invoicesRoot,
        block0.score,
        block0.seal
    );
    const header1 = new Header(
        header0.hashing(),
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
    const header2 = new Header(
        header1.hashing(),
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
    return [header0, block1, header2];
}

async function setupEach(
    base: number,
    promises: PromiseExpect
): Promise<[CodeChain, TestHelper]> {
    const node = new CodeChain({ base });
    await promises.shouldFulfill("node.start", node.start());
    const TH = new TestHelper("0.0.0.0", node.port);
    await promises.shouldFulfill("th.establish", TH.establish());
    return [node, TH];
}

async function teardownEach(
    currentTest: Test,
    TH: TestHelper,
    node: CodeChain,
    promises: PromiseExpect
) {
    if (currentTest.state === "failed") {
        node.testFailed(currentTest.fullTitle());
    }
    await promises.shouldFulfill("th.end", TH.end());
    await promises.shouldFulfill("node.clean", node.clean());
}

async function testBody(
    header0: Header,
    block1: Block,
    header2: Header,
    TH: TestHelper,
    params: {
        tparent?: H256;
        ttimeStamp?: U256;
        tnumber?: U256;
        tauthor?: H160;
        textraData?: Buffer;
        ttransactionRoot?: H256;
        tstateRoot?: H256;
        tinvoiceRoot?: H256;
        tscore?: U256;
        tseal?: Buffer[];
    },
    promises: PromiseExpect
) {
    const {
        tnumber,
        textraData,
        tscore,
        tparent,
        tauthor,
        ttransactionRoot,
        tstateRoot,
        tinvoiceRoot,
        tseal
    } = params;

    const bestHash = header2.hashing();
    const bestScore = header2.getScore();

    const header = new Header(
        header0.hashing(),
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

    if (tparent != null) {
        header.setParentHash(tparent);
    }
    if (tnumber != null) {
        header.setNumber(tnumber);
    }
    if (tauthor != null) {
        header.setAuthor(tauthor);
    }
    if (textraData != null) {
        header.setExtraData(textraData);
    }
    if (ttransactionRoot != null) {
        header.setParcelsRoot(ttransactionRoot);
    }
    if (tstateRoot != null) {
        header.setStateRoot(tstateRoot);
    }
    if (tinvoiceRoot != null) {
        header.setInvoiceRoot(tinvoiceRoot);
    }
    if (tscore != null) {
        header.setScore(tscore);
    }
    if (tseal != null) {
        header.setSeal(tseal);
    }

    const genesis = TH.genesisHash;
    await promises.shouldFulfill(
        "th.status",
        TH.sendStatus(bestScore, bestHash, genesis)
    );
    await promises.shouldFulfill(
        "th.header.response.",
        TH.sendBlockHeaderResponse([
            header0.toEncodeObject(),
            header.toEncodeObject(),
            header2.toEncodeObject()
        ])
    );
    await promises.shouldFulfill("th.header.request.", TH.waitHeaderRequest());

    const bodyRequest = TH.getBlockBodyRequest();

    const _ = expect(bodyRequest).to.be.null;
}

export async function createTestSuite(
    testNumber: number,
    title: string,
    params: any
) {
    // tslint:disable only-arrow-functions
    describe(`invalid block propagation ${testNumber}`, async function() {
        // tslint:enable only-arrow-functions
        let node: CodeChain;
        let TH: TestHelper;
        let header0: Header;
        let block1: Block;
        let header2: Header;

        const BASE = 300 + testNumber * 5;
        const promises = new PromiseExpect();

        // tslint:disable only-arrow-functions
        before(async function() {
            // tslint:enable only-arrow-functions
            [header0, block1, header2] = await setup(BASE, promises);
        });

        // tslint:disable only-arrow-functions
        beforeEach(async function() {
            // tslint:enable only-arrow-functions
            [node, TH] = await setupEach(BASE, promises);
        });

        afterEach(async function() {
            await teardownEach(this.currentTest!, TH, node, promises);
        });

        // tslint:disable only-arrow-functions
        it(title, async function() {
            // tslint:enable only-arrow-functions
            await testBody(header0, block1, header2, TH, params, promises);
        }).timeout(30_000);

        // tslint:disable only-arrow-functions
        after(async function() {
            await promises.checkFulfilled();
        });
    });
}
