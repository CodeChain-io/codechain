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

import CodeChain from "../helper/spawn";
import {
    Timelock,
    Asset,
    AssetTransferAddress
} from "codechain-sdk/lib/core/classes";
import { faucetAddress } from "../helper/constants";
import { H256 } from "codechain-primitives/lib";

import "mocha";
import { expect } from "chai";

describe("Sealing test", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("stopSealing then startSealing", async function() {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendSignedParcel({ awaitInvoice: false });
        expect(await node.getBestBlockNumber()).to.equal(0);
        await node.sdk.rpc.devel.startSealing();
        expect(await node.getBestBlockNumber()).to.equal(1);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});

describe("Future queue", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("all pending parcel must be mined", async function() {
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;

        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 3 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 2 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 1 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(
            seq + 4
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});

describe("Timelock", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            argv: ["--force-sealing", "--no-reseal-timer"]
        });
        await node.start();
    });

    async function checkTx(txhash: H256, shouldBeConfirmed: boolean) {
        const invoices = await node.sdk.rpc.chain.getTransactionInvoices(
            txhash
        );
        if (shouldBeConfirmed) {
            expect(invoices.length).to.equal(1);
            expect(invoices[0].error).to.be.undefined;
            expect(invoices[0].success).to.be.true;
        } else {
            expect(invoices.length).to.equal(0);
        }
    }

    async function sendTransferTx(
        asset: Asset,
        timelock?: Timelock,
        options: {
            fee?: number;
        } = {}
    ): Promise<H256> {
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(
            timelock
                ? asset.createTransferInput({
                      timelock
                  })
                : asset.createTransferInput()
        );
        tx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransactionInput(tx, 0);
        const { fee } = options;
        await node.sendTransaction(tx, { awaitInvoice: false, fee });
        return tx.hash();
    }

    describe("The current items should move to the future queue", async function() {
        it("Minted at block 1, send transfer without timelock and then replace it with Timelock::Block(3)", async function() {
            const { asset } = await node.mintAsset({ amount: 1 });
            await node.sdk.rpc.devel.stopSealing();
            const txhash1 = await sendTransferTx(asset, undefined);
            const txhash2 = await sendTransferTx(
                asset,
                {
                    type: "block",
                    value: 3
                },
                {
                    fee: 20
                }
            );
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
