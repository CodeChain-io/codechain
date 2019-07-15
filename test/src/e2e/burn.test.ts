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
import "mocha";
import { faucetAddress } from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("Burn", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("burn", async function() {
        const asset = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const hash1 = await node.sendAssetTransaction(tx1);
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createTransferAssetTransaction();
        tx2.addBurns(transferredAsset);
        await node.signTransactionBurn(tx2, 0);
        const hash2 = await node.sendAssetTransaction(tx2);
        expect(await node.sdk.rpc.chain.getTransaction(hash2)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;

        expect(
            await node.sdk.rpc.chain.getAsset(tx2.tracker(), 0, asset.shardId)
        ).to.be.null;
    });

    it("burn ZeroQuantity", async function() {
        const asset = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const hash = await node.sendAssetTransaction(tx1);
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;

        const tx2 = node.sdk.core.createTransferAssetTransaction();
        const {
            assetType,
            shardId,
            lockScriptHash,
            parameters
        } = tx1.getTransferredAsset(0);
        tx2.addBurns(
            node.sdk.core.createAssetTransferInput({
                assetOutPoint: {
                    assetType,
                    shardId,
                    tracker: tx1.tracker(),
                    index: 0,
                    lockScriptHash,
                    parameters,
                    quantity: 0
                }
            })
        );
        await node.signTransactionBurn(tx2, 0);
        try {
            await node.sendAssetTransaction(tx2);
            expect.fail();
        } catch (e) {
            expect(e).is.similarTo(ERROR.INVALID_TX_ZERO_QUANTITY);
        }
    });

    it("Cannot transfer P2PKHBurn asset", async function() {
        const asset = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const hash1 = await node.sendAssetTransaction(tx1);
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createTransferAssetTransaction();
        tx2.addInputs(transferredAsset);
        tx2.addOutputs({
            assetType: transferredAsset.assetType,
            shardId: transferredAsset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionP2PKHBurn(
            tx2.input(0)!,
            tx2.hashWithoutScript()
        );

        await node.sendAssetTransactionExpectedToFail(tx2);

        expect(
            await node.sdk.rpc.chain.getAsset(tx1.tracker(), 0, asset.shardId)
        ).not.to.be.null;
        expect(
            await node.sdk.rpc.chain.getAsset(tx2.tracker(), 0, asset.shardId)
        ).be.null;
    });

    it("Cannot burn P2PKH asset", async function() {
        const asset = await node.mintAsset({ supply: 1 });
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addBurns(asset);
        await node.signTransactionP2PKH(tx.burn(0)!, tx.hashWithoutScript());

        await node.sendAssetTransactionExpectedToFail(tx);
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
