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

            expect(
                await node.sdk.rpc.chain.getTransactionResult(hash, {
                    timeout: 120 * 1000
                })
            ).to.be.true;

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
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: hitActionHandlerId,
                        bytes: RLP.encode(["wrong", "format", "of", "message"])
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );

            expect(
                await node.sdk.rpc.chain.getTransactionResult(hash, {
                    timeout: 120 * 1000
                })
            ).to.be.false;
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
