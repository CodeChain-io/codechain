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

import CodeChain from "../helper/spawn";

import "mocha";
import { expect } from "chai";
import {
    hitActionHandlerId,
    faucetSecret,
    faucetAddress
} from "../helper/constants";
import { toHex } from "codechain-primitives/lib";
import { fail } from "assert";
import { errorMatcher, ERROR } from "../helper/error";

const RLP = require("rlp");

describe("engine", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("getCoinbase", async function() {
        // TODO: Coinbase is not defined in solo mode, so it always returns null. Need to test in other modes.
        expect(
            await node.sdk.rpc.sendRpcRequest("engine_getCoinbase", [])
        ).to.be.a("null");
    });

    it("getRecommendedConfirmation", async function() {
        // TODO: The rcommended confirmation of solo is always 1. Need to test in other modes.
        expect(
            await node.sdk.rpc.sendRpcRequest(
                "engine_getRecommendedConfirmation",
                []
            )
        ).to.equal(1);
    });

    describe("getCustomActionData", function() {
        it("should have initial state", async function() {
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

            const invoice = (await node.sdk.rpc.chain.getInvoice(hash, {
                timeout: 120 * 1000
            }))!;

            expect(invoice.error).to.be.undefined;
            expect(invoice.success).to.be.true;

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
                expect(e).to.satisfy(errorMatcher(ERROR.STATE_NOT_EXIST));
            }
        });

        it("should throw hander not found", async function() {
            try {
                await node.sdk.rpc.engine.getCustomActionData(999999, []);
                fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.ACTION_DATA_HANDLER_NOT_FOUND)
                );
            }
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
