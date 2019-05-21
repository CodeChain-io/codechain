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

chai.use(chaiAsPromised);
const expect = chai.expect;

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
                ["hit count"]
            );

            expect(actionData).to.be.equal(toHex(RLP.encode(1)));
        });

        it("should alter state", async function() {
            const previousHitData = (await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["hit count"]
            ))!;
            const previousHitCount = Buffer.from(
                previousHitData,
                "hex"
            ).readUInt8(0);

            const previousBlockCloseData = (await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["close count"]
            ))!;
            const previousBlockCloseCount = Buffer.from(
                previousBlockCloseData,
                "hex"
            ).readUInt8(0);

            const increment = 11;
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: hitActionHandlerId,
                        bytes: RLP.encode([increment])
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );

            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;

            const hitData = await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["hit count"]
            );

            expect(hitData).to.be.equal(
                toHex(RLP.encode(previousHitCount + increment))
            );
            const closeData = await node.sdk.rpc.engine.getCustomActionData(
                hitActionHandlerId,
                ["close count"]
            );
            expect(closeData).to.be.equal(
                toHex(RLP.encode(previousBlockCloseCount + 1))
            );
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
                    ["hit count"],
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

            expect(
                node.sdk.rpc.chain.sendSignedTransaction(
                    node.sdk.core
                        .createCustomTransaction({
                            handlerId: hitActionHandlerId,
                            bytes: RLP.encode([
                                "wrong",
                                "format",
                                "of",
                                "message"
                            ])
                        })
                        .sign({
                            secret: faucetSecret,
                            seq: seq + 1,
                            fee: 10
                        })
                )
            ).be.rejected;
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
