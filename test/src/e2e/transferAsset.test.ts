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

import { Buffer } from "buffer";
import { expect } from "chai";
import {
    Asset,
    AssetAddress,
    H160,
    PlatformAddress,
    Script,
    TransferAsset
} from "codechain-sdk/lib/core/classes";
import { P2PKH } from "codechain-sdk/lib/key/P2PKH";
import { blake160 } from "codechain-sdk/lib/utils";
import * as _ from "lodash";
import "mocha";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("TransferAsset", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("1 input (100 quantity)", async function() {
        let input: Asset;
        const amount = 100;

        beforeEach(async function() {
            const asset = await node.mintAsset({ supply: amount });
            input = asset;
        });

        [[100], [99, 1], [1, 99], Array(100).fill(1)].forEach(function(
            amounts
        ) {
            it(`output amount list: ${amounts}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createTransferAssetTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    amounts.map(quantity => ({
                        assetType: input.assetType,
                        shardId: input.shardId,
                        recipient,
                        quantity
                    }))
                );
                await node.signTransactionInput(tx, 0);

                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            });
        });

        [[0], [99], [101], [100, 100]].forEach(function(amounts) {
            it(`InconsistentTransactionInOut - output amount list: ${amounts}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createTransferAssetTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    amounts.map(quantity => ({
                        assetType: input.assetType,
                        shardId: input.shardId,
                        recipient,
                        quantity
                    }))
                );
                await node.signTransactionInput(tx, 0);
                try {
                    await node.sendAssetTransaction(tx);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INVALID_TX_INCONSISTENT_IN_OUT
                    );
                }
            });
        });

        it("unsuccessful(ZeroAmount) - output amount list: [100, 0]", async function() {
            const amounts = [100, 0];
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs(input);
            tx.addOutputs(
                amounts.map(quantity => ({
                    assetType: input.assetType,
                    shardId: input.shardId,
                    recipient,
                    quantity
                }))
            );
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_ZERO_QUANTITY);
            }
        });

        it("unsuccessful - wrong asset type", async function() {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs(input);
            tx.addOutputs({
                assetType: H160.zero(),
                shardId: input.shardId,
                recipient,
                quantity: amount
            });
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_INCONSISTENT_IN_OUT);
            }
        });

        it("unsuccessful - previous output is duplicated", async function() {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs(input, input);
            tx.addOutputs({
                assetType: input.assetType,
                shardId: input.shardId,
                recipient,
                quantity: amount * 2
            });
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_DUPLICATED_PREV_OUT);
            }
        });
    });

    describe("2 different types of input (10 amount, 20 amount)", async function() {
        let input1: Asset;
        let input2: Asset;
        const amount1 = 10;
        const amount2 = 20;

        beforeEach(async function() {
            let asset = await node.mintAsset({ supply: amount1 });
            input1 = asset;
            asset = await node.mintAsset({ supply: amount2 });
            input2 = asset;
        });

        [
            { input1Amounts: [10], input2Amounts: [20] },
            { input1Amounts: [5, 5], input2Amounts: [10, 10] },
            {
                input1Amounts: [1, 1, 1, 1, 1, 5],
                input2Amounts: [1, 1, 1, 1, 1, 5, 10]
            }
        ].forEach(function(params: {
            input1Amounts: number[];
            input2Amounts: number[];
        }) {
            const { input1Amounts, input2Amounts } = params;

            it(`asset1 ${input1Amounts}, asset2 ${input2Amounts}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createTransferAssetTransaction();
                tx.addInputs(_.shuffle([input1, input2]));
                tx.addOutputs(
                    _.shuffle([
                        ...input1Amounts.map(amount => ({
                            assetType: input1.assetType,
                            shardId: input1.shardId,
                            recipient,
                            quantity: amount
                        })),
                        ...input2Amounts.map(amount => ({
                            assetType: input2.assetType,
                            shardId: input2.shardId,
                            recipient,
                            quantity: amount
                        }))
                    ])
                );
                await node.signTransactionInput(tx, 0);
                await node.signTransactionInput(tx, 1);
                const hash = await node.sendAssetTransaction(tx);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });
        });
    });

    it("Nonexistent assetType", async function() {
        const asset = await node.mintAsset({ supply: 1 });
        const assetType = new H160("0000000000000000000000000000000000123456");
        const transferAsset = node.sdk.core.createTransferAssetTransaction();
        const input = node.sdk.core.createAssetTransferInput({
            assetOutPoint: {
                tracker: asset.outPoint.tracker,
                index: asset.outPoint.index,
                assetType,
                shardId: asset.shardId,
                quantity: asset.quantity,
                lockScriptHash: asset.outPoint.lockScriptHash,
                parameters: asset.outPoint.parameters
            }
        });
        transferAsset.addInputs(input);
        const recipient = await node.sdk.key.createAssetAddress();
        transferAsset.addOutputs({
            quantity: asset.quantity,
            assetType,
            shardId: 0,
            recipient
        });
        await node.signTransactionInput(transferAsset, 0);
        await node.sendAssetTransactionExpectedToFail(transferAsset);
    });

    describe("ScriptError", function() {
        it("Cannot transfer with invalid unlock script", async function() {
            const Opcode = Script.Opcode;
            const asset = await node.mintAsset({ supply: 1 });
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(P2PKH.getLockScript());
            tx.input(0)!.setUnlockScript(Buffer.from([Opcode.NOP])); // Invalid Opcode for unlock_script

            await node.sendAssetTransactionExpectedToFail(tx);
        });

        it("Cannot transfer trivially fail script", async function() {
            const triviallyFail = Buffer.from([0x03]); // Opcode.FAIL
            const asset = await node.mintAsset({
                supply: 1,
                recipient: AssetAddress.fromTypeAndPayload(
                    0,
                    blake160(triviallyFail),
                    {
                        networkId: "tc"
                    }
                )
            });
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(triviallyFail);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            await node.sendAssetTransactionExpectedToFail(tx);
        });

        it("Can transfer trivially success script", async function() {
            const Opcode = Script.Opcode;
            const triviallySuccess = Buffer.from([Opcode.PUSH, 1]);
            const asset = await node.mintAsset({
                supply: 1,
                recipient: AssetAddress.fromTypeAndPayload(
                    0,
                    blake160(triviallySuccess),
                    {
                        networkId: "tc"
                    }
                )
            });
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(triviallySuccess);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            const hash = await node.sendAssetTransaction(tx);
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        });

        it("Cannot transfer when lock script left multiple values in stack", async function() {
            const Opcode = Script.Opcode;
            const leaveMultipleValue = Buffer.from([
                Opcode.PUSH,
                1,
                Opcode.DUP
            ]);
            const asset = await node.mintAsset({
                supply: 1,
                recipient: AssetAddress.fromTypeAndPayload(
                    0,
                    blake160(leaveMultipleValue),
                    {
                        networkId: "tc"
                    }
                )
            });
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(leaveMultipleValue);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            await node.sendAssetTransactionExpectedToFail(tx);
        });
    });

    describe("approver", function() {
        let approver: PlatformAddress;
        let nonApprover: PlatformAddress;
        let transferTx: TransferAsset;
        before(async function() {
            approver = await node.createPlatformAddress();
            nonApprover = await node.createPlatformAddress();
            await node.pay(approver, 10000);
            await node.pay(nonApprover, 10000);
        });

        beforeEach(async function() {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createMintAssetTransaction({
                scheme: {
                    shardId: 0,
                    metadata: "",
                    supply: 10000,
                    approver
                },
                recipient
            });
            await node.sendAssetTransaction(tx);
            const asset = await node.sdk.rpc.chain.getAsset(tx.tracker(), 0, 0);
            if (asset === null) {
                throw Error(`Failed to mint an asset`);
            }
            transferTx = node.sdk.core.createTransferAssetTransaction();
            transferTx.addInputs(asset);
            transferTx.addOutputs({
                assetType: asset.assetType,
                shardId: asset.shardId,
                quantity: 10000,
                recipient: await node.createP2PKHAddress()
            });
            await node.signTransactionInput(transferTx, 0);
        });

        it("approver sends a transaction", async function() {
            const hash = await node.sendTransaction(transferTx, {
                account: approver
            });
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        });

        it("nonApprover cannot send a transaction", async function() {
            await node.sendAssetTransactionExpectedToFail(transferTx);
        });
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
