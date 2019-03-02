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
import { H160, H256, U256 } from "codechain-primitives";
import { Block } from "codechain-sdk/lib/core/Block";
import "mocha";
import Test = Mocha.Test;
import { Mock } from "../helper/mock/";
import { Header } from "../helper/mock/cHeader";
import CodeChain from "../helper/spawn";

async function setup(): Promise<[Header, Block, Header]> {
    const temporaryNode = new CodeChain({
        argv: ["--force-sealing"]
    });
    await temporaryNode.start();

    const sdk = temporaryNode.sdk;

    await sdk.rpc.devel.startSealing();
    await sdk.rpc.devel.startSealing();

    const block0 = await sdk.rpc.chain.getBlock(0);
    if (block0 == null) {
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

    await temporaryNode.clean();
    const header0 = new Header(
        block0.parentHash,
        new U256(block0.timestamp),
        new U256(block0.number),
        block0.author.accountId,
        Buffer.from(block0.extraData),
        block0.transactionsRoot,
        block0.stateRoot,
        block0.resultsRoot,
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
        block1.resultsRoot,
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
        block2.resultsRoot,
        new U256(33333333333333),
        block2.seal
    );
    return [header0, block1, header2];
}

async function setupEach(): Promise<[CodeChain, Mock]> {
    const node = new CodeChain();
    await node.start();
    const mock = new Mock("0.0.0.0", node.port, "tc");
    await mock.establish();
    return [node, mock];
}

async function teardownEach(currentTest: Test, mock: Mock, node: CodeChain) {
    if (currentTest.state === "failed") {
        node.testFailed(currentTest.fullTitle());
    }
    await mock.end();
    await node.clean();
}

async function testBody(
    header0: Header,
    block1: Block,
    header2: Header,
    mock: Mock,
    params: {
        tparent?: H256;
        ttimeStamp?: U256;
        tnumber?: U256;
        tauthor?: H160;
        textraData?: Buffer;
        ttransactionRoot?: H256;
        tstateRoot?: H256;
        tResultsRoot?: H256;
        tscore?: U256;
        tseal?: number[][];
    }
) {
    const {
        tnumber,
        textraData,
        tscore,
        tparent,
        tauthor,
        ttransactionRoot,
        tstateRoot,
        tResultsRoot,
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
        block1.resultsRoot,
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
        header.setTransactionsRoot(ttransactionRoot);
    }
    if (tstateRoot != null) {
        header.setStateRoot(tstateRoot);
    }
    if (tResultsRoot != null) {
        header.setResultsRoot(tResultsRoot);
    }
    if (tscore != null) {
        header.setScore(tscore);
    }
    if (tseal != null) {
        header.setSeal(tseal);
    }

    const genesis = mock.genesisHash;
    await mock.sendStatus(bestScore, bestHash, genesis);
    await mock.sendBlockHeaderResponse([
        header0.toEncodeObject(),
        header.toEncodeObject(),
        header2.toEncodeObject()
    ]);
    await mock.waitHeaderRequest();

    const bodyRequest = mock.getBlockBodyRequest();

    expect(bodyRequest).to.be.null;
}

export function createTestSuite(
    testNumber: number,
    title: string,
    params: any
) {
    // tslint:disable only-arrow-functions
    describe(`invalid block propagation ${testNumber}`, async function() {
        this.timeout(60_000);
        // tslint:enable only-arrow-functions
        let node: CodeChain;
        let mock: Mock;
        let header0: Header;
        let block1: Block;
        let header2: Header;

        // tslint:disable only-arrow-functions
        before(async function() {
            // tslint:enable only-arrow-functions
            [header0, block1, header2] = await setup();
        });

        // tslint:disable only-arrow-functions
        beforeEach(async function() {
            // tslint:enable only-arrow-functions
            [node, mock] = await setupEach();
        });

        afterEach(async function() {
            await teardownEach(this.currentTest!, mock, node);
        });

        // tslint:disable only-arrow-functions
        it(title, async function() {
            // tslint:enable only-arrow-functions
            await testBody(header0, block1, header2, mock, params);
        }).timeout(30_000);
    });
}
