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

import { fail } from "assert";
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import { toHex } from "codechain-primitives/lib";
import "mocha";
import {
    bobAddress,
    faucetAddress,
    faucetSecret,
    hitActionHandlerId
} from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("customAction", function() {
    let node: CodeChain;

    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("customAction", function() {
        it("should get initial state", async function() {
            const actionData = await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["metadata hit"]
            );

            expect(actionData).to.be.equal(toHex(RLP.encode(1)));
        });

        it("should alter state", async function() {
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: hitActionHandlerId,
                        bytes: RLP.encode([11])
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );

            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;

            const actionData = await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["metadata hit"]
            );

            expect(actionData).to.be.equal(toHex(RLP.encode(12)));
        });

        it("should return null", async function() {
            const actionData = await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["non-existing-key"]
            );

            expect(actionData).to.be.null;
        });

        it("should throw state not exist", async function() {
            try {
                await node.sdk.rpc.engine.getCustomActionData(
                    hitActionHandlerId,
                    ["metadata hit"],
                    99999
                );
                fail();
            } catch (e) {
                expect(e).similarTo(ERROR.STATE_NOT_EXIST);
            }
        });

        it("should throw handler not found on getCustomActionData", async function() {
            try {
                await node.sdk.rpc.engine.getCustomActionData(999999, []);
                fail();
            } catch (e) {
                expect(e).similarTo(ERROR.ACTION_DATA_HANDLER_NOT_FOUND);
            }
        });

        it("should throw handler not found on sendCustomTransaction", async function() {
            try {
                await node.sdk.rpc.chain.sendSignedTransaction(
                    node.sdk.core
                        .createCustomTransaction({
                            handlerId: 99999,
                            bytes: RLP.encode([11])
                        })
                        .sign({
                            secret: faucetSecret,
                            seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                            fee: 10
                        })
                );
                fail();
            } catch (e) {
                expect(e).similarTo(ERROR.ACTION_DATA_HANDLER_NOT_FOUND);
            }
        });

        it("should fail on handling error", async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();

            await node.sdk.rpc.devel.stopSealing();
            const hash1 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createPayTransaction({
                        recipient: bobAddress,
                        quantity: 1
                    })
                    .sign({
                        secret: faucetSecret,
                        seq,
                        fee: 10
                    })
            );
            const hash2 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: hitActionHandlerId,
                        bytes: RLP.encode(["wrong", "format", "of", "message"])
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: seq + 1,
                        fee: 10
                    })
            );
            await node.sdk.rpc.devel.startSealing();
            await node.waitBlockNumber(blockNumber + 1);

            const block = (await node.sdk.rpc.chain.getBlock(blockNumber + 1))!;
            expect(block).not.be.null;
            expect(block.transactions.length).equal(1);
            expect(block.transactions[0].hash().value).equal(hash1.value);
            expect(await node.sdk.rpc.chain.getErrorHint(hash2)).not.be.null;
        });
    });

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
    });

    after(async function() {
        await node.clean();
    });
});
