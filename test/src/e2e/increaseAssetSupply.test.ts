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
import { faucetAddress, faucetSecret } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("IncreaseAssetSupply", async function() {
    let outsider: PlatformAddress;
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();

        outsider = await node.createPlatformAddress();
        await node.pay(outsider, 10000);
    });

    it("can increase the total supply", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const tx = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress(),
            supply: increasedAmount
        });

        const hash = await node.sendAssetTransaction(tx);
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;

        const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByType(
            asset.assetType,
            asset.shardId
        );
        expect(assetScheme!.supply.toString(10)).equal(
            new U64(amount + increasedAmount).toString(10)
        );

        const additionalAsset = await node.sdk.rpc.chain.getAsset(
            tx.tracker(),
            0,
            asset.shardId
        );
        expect(additionalAsset!.quantity.toString(10)).equal(
            new U64(increasedAmount).toString(10)
        );
    });

    it("cannot increase supply with the same transaction", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const recipient = await node.createP2PKHAddress();
        const tx1 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        const tx2 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        expect(tx1.tracker().value).equal(tx2.tracker().value);

        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);

        const hash1 = await node.sendAssetTransaction(tx1, { seq });
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;

        await node.sendAssetTransactionExpectedToFail(tx2, { seq: seq + 1 });
    });

    it("cannot increase supply with the same transaction even the asset is moved", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const recipient = await node.createP2PKHAddress();
        const tx1 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        const tx2 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        expect(tx1.tracker().value).equal(tx2.tracker().value);

        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);

        const hash1 = await node.sendAssetTransaction(tx1, { seq });
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;

        const input = tx1.getMintedAsset().createTransferInput();
        const transfer = await node.sdk.core.createTransferAssetTransaction({
            inputs: [input],
            outputs: [
                node.sdk.core.createAssetTransferOutput({
                    assetType: input.prevOut.assetType,
                    shardId: input.prevOut.shardId,
                    quantity: input.prevOut.quantity,
                    recipient
                })
            ]
        });
        await node.signTransactionInput(transfer, 0);
        const transferHash = await node.sdk.rpc.chain.sendSignedTransaction(
            transfer.sign({
                secret: faucetSecret,
                fee: 100,
                seq: seq + 1
            })
        );
        expect(await node.sdk.rpc.chain.containsTransaction(transferHash)).be
            .true;
        expect(await node.sdk.rpc.chain.getTransaction(transferHash)).be.not
            .null;

        await node.sendAssetTransactionExpectedToFail(tx2, { seq: seq + 2 });
    });

    it("can increase supply again", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const recipient = await node.createP2PKHAddress();
        const tx1 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        const tx2 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            seq: 1,
            supply: increasedAmount
        });
        expect(tx1.tracker().value).not.equal(tx2.tracker().value);

        const hash1 = await node.sendAssetTransaction(tx1);
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;

        const hash2 = await node.sendAssetTransaction(tx2);
        expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash2)).not.null;
    });

    it("can increase supply again after move", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const recipient = await node.createP2PKHAddress();
        const tx1 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            supply: increasedAmount
        });
        const tx2 = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient,
            seq: 1,
            supply: increasedAmount
        });
        expect(tx1.tracker().value).not.equal(tx2.tracker().value);

        const hash1 = await node.sendAssetTransaction(tx1);
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;

        const input = tx1.getMintedAsset().createTransferInput();
        const transfer = await node.sdk.core.createTransferAssetTransaction({
            inputs: [input],
            outputs: [
                node.sdk.core.createAssetTransferOutput({
                    assetType: input.prevOut.assetType,
                    shardId: input.prevOut.shardId,
                    quantity: input.prevOut.quantity,
                    recipient
                })
            ]
        });

        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);

        await node.signTransactionInput(transfer, 0);
        const transferHash = await node.sdk.rpc.chain.sendSignedTransaction(
            transfer.sign({
                secret: faucetSecret,
                fee: 100,
                seq
            })
        );
        expect(await node.sdk.rpc.chain.containsTransaction(transferHash)).be
            .true;
        expect(await node.sdk.rpc.chain.getTransaction(transferHash)).be.not
            .null;

        const blockNumber = await node.getBestBlockNumber();
        const hash2 = await node.sendAssetTransaction(tx2);
        await node.waitBlockNumber(blockNumber + 1);

        expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash2)).be.not.null;
    });

    it("cannot increase without registrar", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount
        });
        const tx = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress(),
            supply: increasedAmount
        });

        await node.sendAssetTransactionExpectedToFail(tx);

        const additionalAsset = await node.sdk.rpc.chain.getAsset(
            tx.tracker(),
            0,
            asset.shardId
        );
        expect(additionalAsset).to.be.null;
    });

    it("outsider cannot increase", async function() {
        const amount = 100;
        const increasedAmount = 300;
        const asset = await node.mintAsset({
            supply: amount,
            registrar: faucetAddress
        });
        const tx = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress(),
            supply: increasedAmount
        });

        await node.sendTransactionExpectedToFail(tx, { account: outsider });

        const results = await node.sdk.rpc.chain.getTransactionResultsByTracker(
            tx.tracker(),
            {
                timeout: 300 * 1000
            }
        );
        expect(results).deep.equal([false]);

        const additionalAsset = await node.sdk.rpc.chain.getAsset(
            tx.tracker(),
            0,
            asset.shardId
        );
        expect(additionalAsset).to.be.null;
    });

    it("cannot be overflowed", async function() {
        const asset = await node.mintAsset({
            supply: U64.MAX_VALUE,
            registrar: faucetAddress
        });
        const tx = node.sdk.core.createIncreaseAssetSupplyTransaction({
            shardId: asset.shardId,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress(),
            supply: 1
        });

        await node.sendAssetTransactionExpectedToFail(tx);

        const additionalAsset = await node.sdk.rpc.chain.getAsset(
            tx.tracker(),
            0,
            asset.shardId
        );
        expect(additionalAsset).to.be.null;
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});
