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

import { Block } from "codechain-sdk/lib/core/Block";
import { TestHelper } from "codechain-test-helper/lib/testHelper";
import { Header } from "codechain-test-helper/lib/cHeader";
import CodeChain from "../helper/spawn";
import { H256, U256, H160 } from "codechain-primitives/lib";

import "mocha";
import { expect } from "chai";

describe("Test onChain block communication", async function() {
    let nodeA: CodeChain;
    let TH: TestHelper;
    let soloGenesisBlock: Header;
    let soloBlock1: Block;
    let soloHeader2: Header;

    const INVALID_PARENT = new H256(
        "0x1111111111111111111111111111111111111111111111111111111111111111"
    );
    const INVALID_AUTHOR = new H160(
        "0xffffffffffffffffffffffffffffffffffffffff"
    );
    const INVALID_PARCELROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    const INVALID_STATEROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    const INVALID_INVOICEROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    const INVALID_SEAL = [Buffer.from("DEADBEEF")];

    const INVALID_NUMBER = new U256(2);
    const INVALID_EXTRADATA = Buffer.from("DEADBEEF");
    const INVALID_SCORE = new U256(9999999999999999999999999999999999999999);

    const BASE = 300;

    const testArray = [
        {
            testName: "OnChain invalid parent block propagation test",
            tparent: INVALID_PARENT
        },
        /*
            "OnChain invalid timestamp block propagation test",
        ,*/
        {
            testName: "OnChain invalid number block propagation test",
            tnumber: INVALID_NUMBER
        },
        {
            testName: "OnChain invalid author block propagation test",
            tauthor: INVALID_AUTHOR
        },
        {
            testName: "OnChain invalid extraData block propagation test",
            textraData: INVALID_EXTRADATA
        },
        {
            testName: "OnChain invalid parcelRoot block propagation test",
            tparcelRoot: INVALID_PARCELROOT
        },
        {
            testName: "OnChain invalid stateRoot block propagation test",
            tstateRoot: INVALID_STATEROOT
        },
        {
            testName: "OnChain invalid invoiceRoot block propagation test",
            tinvoiceRoot: INVALID_INVOICEROOT
        },
        {
            testName: "OnChain invalid score block propagation test",
            tscore: INVALID_SCORE
        },
        {
            testName: "OnChain invalid seal block propagation test",
            tseal: INVALID_SEAL
        }
    ];

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
        soloBlock1 = block1;
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
            genesisBlock.parcelsRoot,
            genesisBlock.stateRoot,
            genesisBlock.invoicesRoot,
            genesisBlock.score,
            genesisBlock.seal
        );
        const soloHeader1 = new Header(
            soloGenesisBlock.hashing(),
            new U256(soloBlock1.timestamp),
            new U256(soloBlock1.number),
            soloBlock1.author.accountId,
            Buffer.from(soloBlock1.extraData),
            soloBlock1.parcelsRoot,
            soloBlock1.stateRoot,
            soloBlock1.invoicesRoot,
            new U256(2222222222222),
            soloBlock1.seal
        );
        soloHeader2 = new Header(
            soloHeader1.hashing(),
            new U256(block2.timestamp),
            new U256(block2.number),
            block2.author.accountId,
            Buffer.from(block2.extraData),
            block2.parcelsRoot,
            block2.stateRoot,
            block2.invoicesRoot,
            new U256(33333333333333),
            block2.seal
        );
    });

    beforeEach(async function() {
        nodeA = new CodeChain({ base: BASE });
        await nodeA.start();
        TH = new TestHelper("0.0.0.0", nodeA.port);
        await TH.establish();
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodeA.testFailed(this.currentTest!.fullTitle());
        }
        await TH.end();
        await nodeA.clean();
    });

    describe("OnChain invalid block test", async function() {
        testArray.forEach(function(params: {
            testName: string;
            tparent?: H256;
            ttimeStamp?: U256;
            tnumber?: U256;
            tauthor?: H160;
            textraData?: Buffer;
            tparcelRoot?: H256;
            tstateRoot?: H256;
            tinvoiceRoot?: H256;
            tscore?: U256;
            tseal?: Buffer[];
        }) {
            const { testName } = params;
            const {
                tnumber,
                textraData,
                tscore,
                tparent,
                tauthor,
                tparcelRoot,
                tstateRoot,
                tinvoiceRoot,
                tseal
            } = params;

            it(testName, async function() {
                const bestHash = soloHeader2.hashing();
                const bestScore = soloHeader2.getScore();

                const header = new Header(
                    soloGenesisBlock.hashing(),
                    new U256(soloBlock1.timestamp),
                    new U256(soloBlock1.number),
                    soloBlock1.author.accountId,
                    Buffer.from(soloBlock1.extraData),
                    soloBlock1.parcelsRoot,
                    soloBlock1.stateRoot,
                    soloBlock1.invoicesRoot,
                    new U256(2222222222222),
                    soloBlock1.seal
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
                if (tparcelRoot != null) {
                    header.setParcelsRoot(tparcelRoot);
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
                await TH.sendStatus(bestScore, bestHash, genesis);
                await TH.sendBlockHeaderResponse([
                    soloGenesisBlock.toEncodeObject(),
                    header.toEncodeObject(),
                    soloHeader2.toEncodeObject()
                ]);
                await TH.waitHeaderRequest();

                const bodyRequest = TH.getBlockBodyRequest();

                expect(bodyRequest).to.be.null;
            }).timeout(30_000);
        });
    });
});
