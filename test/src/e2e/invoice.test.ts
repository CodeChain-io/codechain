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

describe("invoice", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("Invoice of Mint Asset", async function() {
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

        const mintInvoices = await node.sdk.rpc.chain.getInvoicesByTracker(
            mint.tracker()
        );
        expect(mintInvoices).not.to.be.null;
        expect(mintInvoices.length).to.equal(1);
        expect(mintInvoices[0]).to.be.true;
        const mintInvoice = (await node.sdk.rpc.chain.getInvoice(
            signedMint.hash()
        ))!;
        expect(mintInvoice).not.to.be.null;
        expect(mintInvoice).to.be.true;
    });

    it("Invoice of Transfer Asset", async function() {
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

        const mintInvoices = await node.sdk.rpc.chain.getInvoicesByTracker(
            mint.tracker()
        );
        expect(mintInvoices).not.to.be.null;
        expect(mintInvoices.length).to.equal(1);
        expect(mintInvoices[0]).to.be.true;

        const transferInvoices = await node.sdk.rpc.chain.getInvoicesByTracker(
            transfer2.tracker()
        );
        expect(transferInvoices).not.to.be.null;
        expect(transferInvoices.length).to.equal(2);
        expect(transferInvoices[0]).to.be.false;
        expect(transferInvoices[1]).to.be.true;

        const transfer1Invoice = (await node.sdk.rpc.chain.getInvoice(
            signedTransfer1.hash()
        ))!;
        expect(transfer1Invoice).not.to.be.null;
        expect(transfer1Invoice).to.be.false;

        const mintInvoice = (await node.sdk.rpc.chain.getInvoice(
            signedMint.hash()
        ))!;
        expect(mintInvoice).not.to.be.null;
        expect(mintInvoice).to.be.true;

        const transfer2Invoice = (await node.sdk.rpc.chain.getInvoice(
            signedTransfer2.hash()
        ))!;
        expect(transfer2Invoice).not.to.be.null;
        expect(transfer2Invoice).to.be.true;
    });

    describe("In the same block", async function() {
        it("Invoice of Transfer Asset", async function() {
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

            const mintInvoices = await node.sdk.rpc.chain.getInvoicesByTracker(
                mint.tracker()
            );
            expect(mintInvoices).not.to.be.null;
            expect(mintInvoices.length).to.equal(1);
            expect(mintInvoices[0]).to.be.true;

            const transferInvoices = await node.sdk.rpc.chain.getInvoicesByTracker(
                transfer2.tracker()
            );
            expect(transferInvoices).not.to.be.null;
            expect(transferInvoices.length).to.equal(2);
            expect(transferInvoices[0]).to.be.false;
            expect(transferInvoices[1]).to.be.true;

            const transfer1Invoice = (await node.sdk.rpc.chain.getInvoice(
                signedTransfer1.hash()
            ))!;
            expect(transfer1Invoice).not.to.be.null;
            expect(transfer1Invoice).to.be.false;

            const mintInvoice = (await node.sdk.rpc.chain.getInvoice(
                signedMint.hash()
            ))!;
            expect(mintInvoice).not.to.be.null;
            expect(mintInvoice).to.be.true;

            const transfer2Invoice = (await node.sdk.rpc.chain.getInvoice(
                signedTransfer2.hash()
            ))!;
            expect(transfer2Invoice).not.to.be.null;
            expect(transfer2Invoice).to.be.true;

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
        });
        after(async function() {
            await node.sdk.rpc.devel.startSealing();
        });
    });

    after(async function() {
        await node.clean();
    });
});
