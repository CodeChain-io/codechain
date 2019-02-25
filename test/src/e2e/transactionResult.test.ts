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
import { H160, H256, H512, U64 } from "codechain-sdk/lib/core/classes";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
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
        expect(await node.sdk.rpc.chain.getTransactionResult(signedMint.hash()))
            .to.be.true;

        expect(
            (await node.sdk.rpc.chain.getTransaction(signedMint.hash()))!.result
        ).to.be.true;

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const bestBlock = (await node.sdk.rpc.chain.getBlock(bestBlockNumber))!;
        expect(bestBlock).not.to.be.null;
        expect(bestBlock.transactions[0].result).to.be.true;
    });

    it("of Transfer Asset", async function() {
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
        const signedTransfer1 = transfer1.sign({
            secret: faucetSecret,
            fee: 10,
            seq
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
        await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer1);
        await node.sdk.rpc.chain.sendSignedTransaction(signedMint);
        await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer2);

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
            await node.sdk.rpc.chain.getTransactionResult(
                signedTransfer1.hash()
            )
        ).to.be.false;
        expect(
            (await node.sdk.rpc.chain.getTransaction(signedTransfer1.hash()))!
                .result
        ).to.be.false;

        expect(await node.sdk.rpc.chain.getTransactionResult(signedMint.hash()))
            .to.be.true;
        expect(
            (await node.sdk.rpc.chain.getTransaction(signedMint.hash()))!.result
        ).to.be.true;

        expect(
            await node.sdk.rpc.chain.getTransactionResult(
                signedTransfer2.hash()
            )
        ).to.be.true;
        expect(
            (await node.sdk.rpc.chain.getTransaction(signedTransfer2.hash()))!
                .result
        ).to.be.true;

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const transferBlock1 = (await node.sdk.rpc.chain.getBlock(
            bestBlockNumber - 2
        ))!;
        expect(transferBlock1).not.to.be.null;
        expect(transferBlock1.transactions[0].result).to.be.false;

        const mintBlock = (await node.sdk.rpc.chain.getBlock(
            bestBlockNumber - 1
        ))!;
        expect(mintBlock).not.to.be.null;
        expect(mintBlock.transactions[0].result).to.be.true;

        const transferBlock2 = (await node.sdk.rpc.chain.getBlock(
            bestBlockNumber
        ))!;
        expect(transferBlock2).not.to.be.null;
        expect(transferBlock2.transactions[0].result).to.be.true;
    });

    describe("In the same block", async function() {
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
            const signedTransfer1 = transfer1.sign({
                secret: faucetSecret,
                fee: 10,
                seq
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
            await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer1);
            await node.sdk.rpc.chain.sendSignedTransaction(signedMint);
            await node.sdk.rpc.chain.sendSignedTransaction(signedTransfer2);
            await node.sdk.rpc.devel.startSealing();
            await node.waitBlockNumber(blockNumberBeforeTx + 1);

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
                await node.sdk.rpc.chain.getTransactionResult(
                    signedTransfer1.hash()
                )
            ).to.be.false;
            expect(
                (await node.sdk.rpc.chain.getTransaction(
                    signedTransfer1.hash()
                ))!.result
            ).to.be.false;

            expect(
                await node.sdk.rpc.chain.getTransactionResult(signedMint.hash())
            ).to.be.true;
            expect(
                (await node.sdk.rpc.chain.getTransaction(signedMint.hash()))!
                    .result
            ).to.be.true;

            expect(
                await node.sdk.rpc.chain.getTransactionResult(
                    signedTransfer2.hash()
                )
            ).to.be.true;
            expect(
                (await node.sdk.rpc.chain.getTransaction(
                    signedTransfer2.hash()
                ))!.result
            ).to.be.true;

            const block = (await node.sdk.rpc.chain.getBlock(
                blockNumberBeforeTx + 1
            ))!;
            expect(block).not.to.be.null;
            expect(block.transactions.length).to.equal(3);
            expect(block.transactions[0].hash().value).to.equal(
                signedTransfer1.hash().value
            );
            expect(block.transactions[1].hash().value).to.equal(
                signedMint.hash().value
            );
            expect(block.transactions[2].hash().value).to.equal(
                signedTransfer2.hash().value
            );
            expect(block.transactions[0].result).to.be.false;
            expect(block.transactions[1].result).to.be.true;
            expect(block.transactions[2].result).to.be.true;
        });
        after(async function() {
            await node.sdk.rpc.devel.startSealing();
        });
    });

    after(async function() {
        await node.clean();
    });
});
