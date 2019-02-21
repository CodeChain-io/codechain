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

import { fail } from "assert";
import { expect } from "chai";
import { toHex } from "codechain-primitives/lib";
import "mocha";
import {
    faucetAddress,
    faucetSecret,
    hitActionHandlerId
} from "../helper/constants";
import { ERROR } from "../helper/error";

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

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
    });

    after(async function() {
        await node.clean();
    });
});
