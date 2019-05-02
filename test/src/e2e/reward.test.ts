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
import { PlatformAddress, U64 } from "codechain-sdk/lib/core/classes";
import "mocha";
import {
    aliceAddress,
    aliceSecret,
    bobAddress,
    faucetAddress
} from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("Reward = 50, 1 miner", function() {
    const MIN_FEE_PAY = 10;
    const BLOCK_REWARD = 50;
    const FAUCET_INITIAL_CCS = new U64("18000000000000000000");

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
        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(FAUCET_INITIAL_CCS);
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(0)
        );
    });

    it("Mining a block with 1 transaction", async function() {
        await node.sendPayTx({ fee: 10 });

        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(
            FAUCET_INITIAL_CCS.minus(10 /* fee */).plus(7 /* share */)
        );
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(2 /* share */).plus(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(1 /* share */)
        );
    });

    it("Mining a block with 3 transactions", async function() {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendPayTx({
            fee: 10,
            seq: 0
        });
        await node.sendPayTx({
            fee: 10,
            seq: 1
        });
        await node.sendPayTx({
            fee: 15,
            seq: 2
        });
        await node.sdk.rpc.devel.startSealing();

        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(
            FAUCET_INITIAL_CCS.minus(10 + 10 + 15 /* fee */).plus(
                21 /* share */
            )
        );
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(6 /* share */)
                .plus(10 + 10 + 15 - 3 * MIN_FEE_PAY /* share remaining */)
                .plus(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(3 /* share */)
        );
    });

    it("Mining a block with a transaction that pays the author", async function() {
        await node.pay(aliceAddress, 100);
        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(
            FAUCET_INITIAL_CCS.minus(100 /* pay */)
                .minus(10 /* fee */)
                .plus(7 /* share */)
        );
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(100 /* pay */).plus(2 /* share */).plus(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(1 /* share */)
        );
    });

    it("Mining a block with a transaction which author pays someone in", async function() {
        await node.sendPayTx({ fee: 10 });
        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(
            FAUCET_INITIAL_CCS.minus(10 /* fee */).plus(7 /* share */)
        );
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(2 /* share */).plus(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(1 /* share */)
        );

        const tx = await node.sdk.core
            .createPayTransaction({
                recipient: faucetAddress,
                quantity: 20
            })
            .sign({ secret: aliceSecret, seq: 0, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(tx);

        expect(
            await node.sdk.rpc.chain.getBalance(faucetAddress)
        ).to.deep.equal(
            FAUCET_INITIAL_CCS.minus(10)
                .plus(7)
                .plus(20 /* pay */)
                .plus(7 /* share */)
        );
        expect(await node.sdk.rpc.chain.getBalance(aliceAddress)).to.deep.equal(
            new U64(2)
                .plus(BLOCK_REWARD)
                .minus(20 /* pay */)
                .minus(10 /* fee */)
                .plus(2 /* share */)
                .plus(BLOCK_REWARD)
        );
        expect(await node.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(1 /* share*/).plus(1 /* share */)
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
