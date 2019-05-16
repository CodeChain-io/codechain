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

import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import {
    Asset,
    AssetAddress,
    H256,
    Transaction
} from "codechain-sdk/lib/core/classes";
import * as _ from "lodash";
import "mocha";
import { faucetAddress } from "../helper/constants";
import CodeChain from "../helper/spawn";
import { AssetTransaction } from "codechain-sdk/lib/core/Transaction";

describe("Partial signature", function() {
    let node: CodeChain;

    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    let assets: Asset[];
    let assetType: H256;
    let address1: AssetAddress;
    let address2: AssetAddress;
    let burnAddress1: AssetAddress;
    let burnAddress2: AssetAddress;
    beforeEach(async function() {
        address1 = await node.sdk.key.createAssetAddress({
            type: "P2PKH"
        });
        address2 = await node.sdk.key.createAssetAddress({
            type: "P2PKH"
        });
        burnAddress1 = await node.sdk.key.createAssetAddress({
            type: "P2PKHBurn"
        });
        burnAddress2 = await node.sdk.key.createAssetAddress({
            type: "P2PKHBurn"
        });
        const mintTx = node.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId: 0,
                metadata: "",
                supply: 4000
            },
            recipient: address1
        });
        const asset = mintTx.getMintedAsset();
        ({ assetType } = asset);
        const transferTx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(asset)
            .addOutputs(
                {
                    assetType,
                    quantity: 1000,
                    recipient: address1,
                    shardId: 0
                },
                {
                    assetType,
                    quantity: 1000,
                    recipient: address2,
                    shardId: 0
                },
                {
                    assetType,
                    quantity: 1000,
                    recipient: burnAddress1,
                    shardId: 0
                },
                {
                    assetType,
                    quantity: 1000,
                    recipient: burnAddress2,
                    shardId: 0
                }
            );
        await node.sdk.key.signTransactionInput(transferTx, 0);
        assets = transferTx.getTransferredAssets();
        await node.sendAssetTransaction(mintTx);
        await node.sendAssetTransaction(transferTx);
    });

    it("Can't add burns after signing with the signature tag of all inputs", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 1000,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0);
        tx.addBurns(assets[2]);
        await node.sdk.key.signTransactionBurn(tx, 0);

        await node.sendAssetTransactionExpectedToFail(tx);
    });

    it("Can add burns after signing with the signature tag of single input", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 1000,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0, {
            signatureTag: { input: "single", output: "all" }
        });
        tx.addBurns(assets[2]);
        await node.sdk.key.signTransactionBurn(tx, 0);
        const hash = await node.sendAssetTransaction(tx);
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
    });

    it("Can't add inputs after signing with the signature tag of all inputs when signing burn", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addBurns(assets[2])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 1000,
                recipient: address1
            });
        await node.sdk.key.signTransactionBurn(tx, 0);
        tx.addInputs(assets[0]);
        await node.sdk.key.signTransactionInput(tx, 0);

        await node.sendAssetTransactionExpectedToFail(tx);
    });

    it("Can add inputs after signing with the signature tag of signle input when signing burn", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addBurns(assets[2])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 1000,
                recipient: address1
            });
        await node.sdk.key.signTransactionBurn(tx, 0, {
            signatureTag: { input: "single", output: "all" }
        });
        tx.addInputs(assets[0]);
        await node.sdk.key.signTransactionInput(tx, 0);

        const hash = await node.sendAssetTransaction(tx);
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
    });

    // FIXME: (WIP) It fails
    it("Can't add inputs after signing with the signature tag of all inputs", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 2000,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0);
        tx.addInputs(assets[1]);
        await node.sdk.key.signTransactionInput(tx, 1);

        await node.sendAssetTransactionExpectedToFail(tx);
    });

    it("Can add inputs after signing with the signature tag of single input", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 2000,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0, {
            signatureTag: { input: "single", output: "all" }
        });
        tx.addInputs(assets[1]);
        await node.sdk.key.signTransactionInput(tx, 1);
        const hash = await node.sendAssetTransaction(tx);
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
    });

    it("Can't add outputs after signing the signature tag of all outputs", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 500,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0);
        tx.addOutputs({
            assetType,
            shardId: 0,
            quantity: 500,
            recipient: address2
        });

        await node.sendAssetTransactionExpectedToFail(tx);
    });

    it("Can add outputs after signing the signature tag of some outputs", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs({
                assetType,
                shardId: 0,
                quantity: 500,
                recipient: address1
            });
        await node.sdk.key.signTransactionInput(tx, 0, {
            signatureTag: {
                input: "all",
                output: [0]
            }
        });
        tx.addOutputs({
            assetType,
            shardId: 0,
            quantity: 500,
            recipient: address2
        });

        const blockNumber = await node.getBestBlockNumber();
        const hash = await node.sendAssetTransaction(tx);
        await node.waitBlockNumber(blockNumber + 1);
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
    });

    it("Can only change the output protected by signature", async function() {
        const tx = node.sdk.core
            .createTransferAssetTransaction()
            .addInputs(assets[0])
            .addOutputs(
                {
                    assetType,
                    shardId: 0,
                    quantity: 500,
                    recipient: address1
                },
                {
                    assetType,
                    shardId: 0,
                    quantity: 500,
                    recipient: address2
                }
            );
        await node.sdk.key.signTransactionInput(tx, 0, {
            signatureTag: {
                input: "all",
                output: [0]
            }
        });
        const address1Param = (tx as any)._transaction.outputs[0].parameters;
        const address2Param = (tx as any)._transaction.outputs[1].parameters;
        ((tx as any)._transaction.outputs[0].parameters as any) = address2Param;

        await node.sendAssetTransactionExpectedToFail(tx);

        ((tx as any)._transaction.outputs[0].parameters as any) = address1Param;
        // FIXME
        (tx as any)._fee = null;
        (tx as any)._seq = null;
        await node.sdk.key.signTransactionInput(tx, 0, {
            signatureTag: {
                input: "all",
                output: [0]
            }
        });

        ((tx as any)._transaction.outputs[1].parameters as any) = address1Param;
        const hash2 = await node.sendAssetTransaction(tx);
        expect(await node.sdk.rpc.chain.getTransaction(hash2)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;
    });

    describe("many outputs", function() {
        [5, 10, 100, 504].forEach(function(length) {
            it(`${length} + 1 outputs`, async function() {
                const tx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(assets[0])
                    .addOutputs(
                        _.times(length, () => ({
                            assetType,
                            shardId: 0,
                            quantity: 1,
                            recipient: address1
                        }))
                    );

                await node.sdk.key.signTransactionInput(tx, 0, {
                    signatureTag: {
                        input: "all",
                        output: _.range(length)
                    }
                });
                tx.addOutputs({
                    assetType,
                    shardId: 0,
                    quantity: 1000 - length,
                    recipient: address1
                });
                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            }).timeout(length * 10 + 5_000);
        });
    });

    describe("dynamic inputs and outputs", function() {
        let splitedAssets: Asset[];
        const splitCount = 50;

        beforeEach(async function() {
            const splitTx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs(
                    _.times(splitCount, () => ({
                        assetType,
                        shardId: 0,
                        quantity: 1000 / splitCount,
                        recipient: address1
                    }))
                );

            await node.sdk.key.signTransactionInput(splitTx, 0);
            splitedAssets = splitTx.getTransferredAssets();
            await node.sendAssetTransaction(splitTx);
        });
        [5, 10, 20].forEach(function(length) {
            it(`${length} inputs and outputs : one-to-one sign`, async function() {
                const tx = node.sdk.core.createTransferAssetTransaction();
                for (let i = 0; i < length; i++) {
                    tx.addInputs(splitedAssets[i]).addOutputs({
                        assetType,
                        shardId: 0,
                        quantity: 1000 / splitCount,
                        recipient: address2
                    });
                    await node.sdk.key.signTransactionInput(tx, i, {
                        signatureTag: {
                            input: "single",
                            output: [i]
                        }
                    });
                }

                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            }).timeout(length * 1000 + 5_000);
        });

        [5, 10, 20].forEach(function(length) {
            it(`${length} inputs and outputs : one-to-many sign`, async function() {
                const tx = node.sdk.core.createTransferAssetTransaction();
                for (let i = 0; i < length; i++) {
                    tx.addInputs(splitedAssets[i]).addOutputs({
                        assetType,
                        shardId: 0,
                        quantity: 1000 / splitCount,
                        recipient: address2
                    });
                    await node.sdk.key.signTransactionInput(tx, i, {
                        signatureTag: {
                            input: "single",
                            output: _.range(i)
                        }
                    });
                }

                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            }).timeout(length * 1000 + 5_000);
        });

        [5, 10, 20].forEach(function(length) {
            it(`${length} burns, inputs and outputs : one-to-many sign`, async function() {
                const tx = node.sdk.core.createTransferAssetTransaction();
                for (let i = 0; i < Math.floor(length / 2); i++) {
                    tx.addInputs(splitedAssets[i]).addOutputs({
                        assetType,
                        shardId: 0,
                        quantity: 1000 / splitCount,
                        recipient: address2
                    });
                    await node.sdk.key.signTransactionInput(tx, i, {
                        signatureTag: {
                            input: "single",
                            output: _.range(i)
                        }
                    });
                }

                tx.addBurns(assets[2]);
                await node.sdk.key.signTransactionBurn(tx, 0, {
                    signatureTag: {
                        input: "single",
                        output: _.range(Math.floor(length / 2))
                    }
                });

                for (let i = Math.floor(length / 2); i < length; i++) {
                    tx.addInputs(splitedAssets[i]).addOutputs({
                        assetType,
                        shardId: 0,
                        quantity: 1000 / splitCount,
                        recipient: address2
                    });
                    await node.sdk.key.signTransactionInput(tx, i, {
                        signatureTag: {
                            input: "single",
                            output: _.range(i)
                        }
                    });
                }

                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            }).timeout(length * 1000 + 5_000);
        });
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
