// Copyright 2019 Kodebox, Inc.
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

import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import "mocha";
import { aliceAddress, faucetAddress, faucetSecret } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("transaction result", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("of Mint Asset", async function() {
        const recipient = await node.createP2PKHAddress();
        const mint = node.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId: 0,
                metadata: "",
                supply: "0xa"
            },
            recipient
        });
        const signedMint = mint.sign({
            secret: faucetSecret,
            fee: 10,
            seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
        });

        await node.sdk.rpc.chain.sendSignedTransaction(signedMint);

        expect(
            await node.sdk.rpc.chain.getTransactionResultsByTracker(
                mint.tracker()
            )
        ).deep.equal([true]);
        expect(await node.sdk.rpc.chain.containsTransaction(signedMint.hash()))
            .be.true;
        expect(await node.sdk.rpc.chain.getTransaction(signedMint.hash())).not
            .null;

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const bestBlock = (await node.sdk.rpc.chain.getBlock(bestBlockNumber))!;
        expect(bestBlock).not.to.be.null;
    });

    it("of Transfer Asset", async function() {
        const blockNumberBeforeTx = await node.sdk.rpc.chain.getBestBlockNumber();
        const mint = node.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId: 0,
                metadata: "",
                supply: "0xa"
            },
            recipient: await node.createP2PKHAddress()
        });

        const asset = mint.getMintedAsset();
        const recipient = await node.createP2PKHAddress();
        const transfer1 = node.sdk.core.createTransferAssetTransaction();
        transfer1.addInputs(asset);
        transfer1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient,
            quantity: 10
        });
        await node.signTransactionInput(transfer1, 0);
        const transfer2 = node.sdk.core.createTransferAssetTransaction();
        transfer2.addInputs(asset);
        transfer2.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient,
            quantity: 10
        });
        await node.signTransactionInput(transfer2, 0);

        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const signedPay = node.sdk.core
            .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
        const signedTransfer1 = transfer1.sign({
            secret: faucetSecret,
            fee: 10,
            seq: seq + 1
        });
        const signedMint = mint.sign({
            secret: faucetSecret,
            fee: 10,
            seq: seq + 1
        });
        const signedTransfer2 = transfer2.sign({
            secret: faucetSecret,
            fee: 10,
            seq: seq + 2
        });

        await node.sdk.rpc.devel.stopSealing();
        // Send pay because the miner doesn't allow the empty block.
        await node.sdk.rpc.chain.sendSignedTransaction(signedPay);
        await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer1);
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumberBeforeTx + 1);

        await node.sdk.rpc.devel.stopSealing();
        await node.sdk.rpc.chain.sendSignedTransaction(signedMint);
        await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer2);
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumberBeforeTx + 2);

        expect(
            await node.sdk.rpc.chain.getTransactionResultsByTracker(
                mint.tracker()
            )
        ).deep.equal([true]);

        expect(
            await node.sdk.rpc.chain.getTransactionResultsByTracker(
                transfer2.tracker()
            )
        ).deep.equal([false, true]);

        expect(
            await node.sdk.rpc.chain.containsTransaction(signedTransfer1.hash())
        ).be.false;
        expect(await node.sdk.rpc.chain.getErrorHint(signedTransfer1.hash()))
            .not.null;

        expect(await node.sdk.rpc.chain.containsTransaction(signedMint.hash()))
            .be.true;
        expect(await node.sdk.rpc.chain.getTransaction(signedMint.hash())).not
            .null;

        expect(
            await node.sdk.rpc.chain.containsTransaction(signedTransfer2.hash())
        ).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(signedTransfer2.hash()))
            .not.null;

        const block1 = (await node.sdk.rpc.chain.getBlock(
            blockNumberBeforeTx + 1
        ))!;
        expect(block1).not.to.be.null;
        expect(block1.transactions.length).to.equal(1);
        expect(block1.transactions[0].hash().value).to.equal(
            signedPay.hash().value
        );

        const block2 = (await node.sdk.rpc.chain.getBlock(
            blockNumberBeforeTx + 2
        ))!;
        expect(block2).not.to.be.null;
        expect(block2.transactions.length).to.equal(2);
        expect(block2.transactions[0].hash().value).to.equal(
            signedMint.hash().value
        );
        expect(block2.transactions[1].hash().value).to.equal(
            signedTransfer2.hash().value
        );
    });

    after(async function() {
        await node.clean();
    });
});
