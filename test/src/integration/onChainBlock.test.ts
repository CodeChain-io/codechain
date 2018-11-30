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
    let soloBlock1: Header;
    let soloBlock2: Header;

    let VALID_PARENT = new H256(
        "ff8324bd3b0232e4fd1799496ae422ee0896cc7a8a64a2885052e320b4ba9535"
    );
    const INVALID_PARENT = new H256(
        "0x1111111111111111111111111111111111111111111111111111111111111111"
    );
    const VALID_TIMESTAMP = new U256(1537944287);
    const INVALID_TIMESTAMP = new U256(1537509962);
    const VALID_NUMBER = new U256(1);
    const INVALID_NUMBER = new U256(2);
    let VALID_AUTHOR = new H160("7777777777777777777777777777777777777777");
    const INVALID_AUTHOR = new H160(
        "0xffffffffffffffffffffffffffffffffffffffff"
    );
    const VALID_EXTRADATA = Buffer.alloc(0);
    const INVALID_EXTRADATA = Buffer.from("DEADBEEF");
    let VALID_PARCELROOT = new H256(
        "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0"
    );
    const INVALID_PARCELROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    let VALID_STATEROOT = new H256(
        "2f6b19afc38f6f1464af20dde08d8bebd6a6aec0a95aaf7ef2fb729c3b88dc5b"
    );
    const INVALID_STATEROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    let VALID_INVOICEROOT = new H256(
        "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0"
    );
    const INVALID_INVOICEROOT = new H256(
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    const VALID_SCORE = new U256(999999999999999);
    const INVALID_SCORE = new U256(9999999999999999999999999999999999999999);
    let VALID_SEAL: Buffer[] = [];
    const INVALID_SEAL = [Buffer.from("DEADBEEF")];

    const testArray = [
        {
            testName: "OnChain invalid parent block propagation test",
            tparent: INVALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        /*
        [
            "OnChain invalid timestamp block propagation test",
            VALID_PARENT,
            INVALID_TIMESTAMP,
            VALID_NUMBER,
            VALID_AUTHOR,
            VALID_EXTRADATA,
            VALID_PARCELROOT,
            VALID_STATEROOT,
            VALID_INVOICEROOT,
            VALID_SCORE,
            VALID_SEAL
        ],*/
        {
            testName: "OnChain invalid number block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: INVALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid author block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: INVALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid extraData block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: INVALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid parcelRoot block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: INVALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid stateRoot block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: INVALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid invoiceRoot block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: INVALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid score block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: INVALID_SCORE,
            tseal: VALID_SEAL
        },
        {
            testName: "OnChain invalid seal block propagation test",
            tparent: VALID_PARENT,
            ttimeStamp: VALID_TIMESTAMP,
            tnumber: VALID_NUMBER,
            tauthor: VALID_AUTHOR,
            textraData: VALID_EXTRADATA,
            tparcelRoot: VALID_PARCELROOT,
            tstateRoot: VALID_STATEROOT,
            tinvoiceRoot: VALID_INVOICEROOT,
            tscore: VALID_SCORE,
            tseal: INVALID_SEAL
        }
    ];

    before(async function() {
        const node = new CodeChain({
            argv: ["--force-sealing"]
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
            genesisBlock.parcelsRoot,
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
            block1.parcelsRoot,
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
            block2.parcelsRoot,
            block2.stateRoot,
            block2.invoicesRoot,
            new U256(33333333333333),
            block2.seal
        );

        VALID_PARENT = block1.parentHash;
        VALID_AUTHOR = block1.author.accountId;
        VALID_PARCELROOT = block1.parcelsRoot;
        VALID_STATEROOT = block1.stateRoot;
        VALID_INVOICEROOT = block1.invoicesRoot;
        VALID_SEAL = block1.seal;
    });

    beforeEach(async function() {
        nodeA = new CodeChain();
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
    }).timeout(30_000);

    describe("OnChain invalid block test", async function() {
        testArray.forEach(function(params: {
            testName: string;
            tparent: H256;
            ttimeStamp: U256;
            tnumber: U256;
            tauthor: H160;
            textraData: Buffer;
            tparcelRoot: H256;
            tstateRoot: H256;
            tinvoiceRoot: H256;
            tscore: U256;
            tseal: Buffer[];
        }) {
            const { testName, tnumber, textraData, tscore } = params;

            it(testName, async function() {
                const tparent = VALID_PARENT;
                const tauthor = VALID_AUTHOR;
                const tparcelRoot = VALID_PARCELROOT;
                const tstateRoot = VALID_STATEROOT;
                const tinvoiceRoot = VALID_INVOICEROOT;
                const tseal = VALID_SEAL;

                // Genesis block
                const header = soloGenesisBlock;

                // Block 1
                const header1 = soloBlock1;

                // Block 2
                const header2 = soloBlock2;

                const bestHash = header2.hashing();
                const bestScore = header2.getScore();

                header1.setParentHash(tparent);
                header1.setNumber(tnumber);
                header1.setAuthor(tauthor);
                header1.setExtraData(textraData);
                header1.setParcelsRoot(tparcelRoot);
                header1.setStateRoot(tstateRoot);
                header1.setInvoiceRoot(tinvoiceRoot);
                header1.setScore(tscore);
                header1.setSeal(tseal);

                const genesis = TH.genesisHash;
                await TH.sendStatus(bestScore, bestHash, genesis);
                await TH.sendBlockHeaderResponse([
                    header.toEncodeObject(),
                    header1.toEncodeObject(),
                    header2.toEncodeObject()
                ]);
                await TH.waitHeaderRequest();

                const bodyRequest = TH.getBlockBodyRequest();

                expect(bodyRequest).to.be.null;
            }).timeout(30_000);
        });
    });
});
