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
import { U64 } from "codechain-sdk/lib/core/classes";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("MintAsset", async function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    [1, 100, U64.MAX_VALUE].forEach(function(supply) {
        it(`Mint successful - supply ${supply}`, async function() {
            const recipient = await node.createP2PKHAddress();
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                supply
            });
            const tx = node.sdk.core.createMintAssetTransaction({
                scheme,
                recipient
            });
            const hash = await node.sendAssetTransaction(tx);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });
    });

    it("Mint unsuccessful - mint supply 0", async function() {
        const scheme = node.sdk.core.createAssetScheme({
            shardId: 0,
            metadata: "",
            supply: 0
        });
        const tx = node.sdk.core.createMintAssetTransaction({
            scheme,
            recipient: await node.createP2PKHAddress()
        });

        try {
            await node.sendAssetTransaction(tx);
            expect.fail();
        } catch (e) {
            expect(e).is.similarTo(ERROR.INVALID_TX_ZERO_QUANTITY);
        }
    });

    it("Mint unsuccessful - mint supply U64.MAX_VALUE + 1", async function() {
        const scheme = node.sdk.core.createAssetScheme({
            shardId: 0,
            metadata: "",
            supply: 0
        });
        (scheme.supply.value as any) = U64.MAX_VALUE.value.plus(1);

        const tx = node.sdk.core.createMintAssetTransaction({
            scheme,
            recipient: await node.createP2PKHAddress()
        });
        const signed = tx.sign({
            secret: faucetSecret,
            seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
            fee: 11
        });

        try {
            await node.sdk.rpc.chain.sendSignedTransaction(signed);
            expect.fail();
        } catch (e) {
            expect(e).is.similarTo(ERROR.INVALID_RLP_TOO_BIG);
        }
    });

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
    });

    after(async function() {
        await node.clean();
    });
});
