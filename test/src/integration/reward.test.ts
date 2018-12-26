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

import { U64 } from "codechain-sdk/lib/core/classes";
import { aliceAddress, aliceSecret, faucetAddress } from "../helper/constants";
import CodeChain from "../helper/spawn";

import "mocha";

import { expect } from "chai";

describe("Reward = 50, 1 miner", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
            argv: ["--author", aliceAddress.toString(), "--force-sealing"]
        });
        await node.start();
    });

    it("Mining an empty block", async function() {
        await node.sdk.rpc.devel.startSealing();
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(50)
        );
    });

    it("Mining a block with 1 transaction", async function() {
        await node.sendPayTx({ fee: 10 });
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(50 + 10)
        );
    });

    it("Mining a block with 3 transactions", async function() {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendPayTx({
            fee: 10,
            seq: 0,
            awaitInvoice: false
        });
        await node.sendPayTx({
            fee: 10,
            seq: 1,
            awaitInvoice: false
        });
        await node.sendPayTx({
            fee: 15,
            seq: 2,
            awaitInvoice: false
        });
        await node.sdk.rpc.devel.startSealing();
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(50 + 35)
        );
    });

    it("Mining a block with a transaction that pays the author", async function() {
        await node.pay(aliceAddress, 100);
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(50 + 10 + 100)
        );
    });

    it("Mining a block with a transaction which author pays someone in", async function() {
        await node.sendPayTx({ fee: 10 }); // +60
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(60)
        );

        const tx = await node.sdk.core
            .createPayTransaction({
                recipient: faucetAddress,
                amount: 50
            })
            .sign({ secret: aliceSecret, seq: 0, fee: 10 }); // -60
        await node.sdk.rpc.chain.sendSignedTransaction(tx); // +60
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(60)
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
