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
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import {
    Asset,
    AssetTransferAddress,
    H160,
    H256,
    MintAsset,
    PlatformAddress,
    Script,
    SignedTransaction,
    TransferAsset,
    U64
} from "codechain-sdk/lib/core/classes";
import { P2PKH } from "codechain-sdk/lib/key/P2PKH";
import { blake160 } from "codechain-sdk/lib/utils";
import * as _ from "lodash";
import "mocha";
import { $anything } from "../helper/chai-similar";
import {
    faucetAccointId,
    faucetAddress,
    faucetSecret
} from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("transactions", function() {
    let node: CodeChain;
    const BASE = 700;
    before(async function() {
        node = new CodeChain({ base: BASE });
        await node.start();
    });

    describe("AssetMint", async function() {
        [1, 100, U64.MAX_VALUE].forEach(function(supply) {
            it(`Mint successful - supply ${supply}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const scheme = node.sdk.core.createAssetScheme({
                    shardId: 0,
                    metadata: "",
                    supply
                });
                const tx = node.sdk.core.createMintAssetTransaction({
                    scheme,
                    recipient
                });
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });
        });

        it("Mint unsuccessful - mint supply 0", async function() {
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                supply: 0
            });
            const tx = node.sdk.core.createMintAssetTransaction({
                scheme,
                recipient: await node.createP2PKHAddress()
            });

            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_ZERO_QUANTITY);
            }
        });

        it("Mint unsuccessful - mint supply U64.MAX_VALUE + 1", async function() {
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                supply: 0
            });
            (scheme.supply.value as any) = U64.MAX_VALUE.value.plus(1);

            const tx = node.sdk.core.createMintAssetTransaction({
                scheme,
                recipient: await node.createP2PKHAddress()
            });
            const signed = tx.sign({
                secret: faucetSecret,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                fee: 11
            });

            try {
                await node.sdk.rpc.chain.sendSignedTransaction(signed);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_RLP_TOO_BIG);
            }
        });
    });

    describe("AssetTransfer - 1 input (100 quantity)", async function() {
        let input: Asset;
        const amount = 100;

        beforeEach(async function() {
            const { asset } = await node.mintAsset({ supply: amount });
            input = asset;
        });

        [[100], [99, 1], [1, 99], Array(100).fill(1)].forEach(function(
            amounts
        ) {
            it(`Transfer successful - output amount list: ${amounts}`, async function() {
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
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });
        });

        [[0], [99], [101], [100, 100]].forEach(function(amounts) {
            it(`Transfer unsuccessful(InconsistentTransactionInOut) - output amount list: ${amounts}`, async function() {
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

        it("Transfer unsuccessful(ZeroAmount) - output amount list: [100, 0]", async function() {
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

        it("Transfer unsuccessful - wrong asset type", async function() {
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

        it("Transfer unsuccessful - previous output is duplicated", async function() {
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

    describe("AssetTransfer - 2 different types of input (10 amount, 20 amount)", async function() {
        let input1: Asset;
        let input2: Asset;
        const amount1 = 10;
        const amount2 = 20;

        beforeEach(async function() {
            let { asset } = await node.mintAsset({ supply: amount1 });
            input1 = asset;
            ({ asset } = await node.mintAsset({ supply: amount2 }));
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

            it(`Transfer successful - asset1 ${input1Amounts}, asset2 ${input2Amounts}`, async function() {
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
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });
        });
    });

    it("Burn successful", async function() {
        const { asset } = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices1 = await node.sendAssetTransaction(tx1);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0]).to.be.true;

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createTransferAssetTransaction();
        tx2.addBurns(transferredAsset);
        await node.signTransactionBurn(tx2, 0);
        const invoices2 = await node.sendAssetTransaction(tx2);
        expect(invoices2!.length).to.equal(1);
        expect(invoices2![0]).to.be.true;

        expect(
            await node.sdk.rpc.chain.getAsset(tx2.tracker(), 0, asset.shardId)
        ).to.be.null;
    });

    it("Burn unsuccessful(ZeroQuantity)", async function() {
        const { asset } = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices = await node.sendAssetTransaction(tx1);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0]).to.be.true;

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
        const { asset } = await node.mintAsset({ supply: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices1 = await node.sendAssetTransaction(tx1);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0]).to.be.true;

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
        const invoices2 = await node.sendAssetTransaction(tx2);
        expect(invoices2!.length).to.equal(1);
        expect(invoices2![0]).to.be.false;
        expect(
            await node.sdk.rpc.chain.getAsset(tx1.tracker(), 0, asset.shardId)
        ).not.to.be.null;
    });

    it("Cannot burn P2PKH asset", async function() {
        const { asset } = await node.mintAsset({ supply: 1 });
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addBurns(asset);
        await node.signTransactionP2PKH(tx.burn(0)!, tx.hashWithoutScript());

        const invoices = await node.sendAssetTransaction(tx);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0]).to.be.false;
    });

    describe("ScriptError", function() {
        it("Cannot transfer with invalid unlock script", async function() {
            const Opcode = Script.Opcode;
            const { asset } = await node.mintAsset({ supply: 1 });
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
        });

        it("Cannot transfer trivially fail script", async function() {
            const triviallyFail = Buffer.from([0x03]); // Opcode.FAIL
            const { asset } = await node.mintAsset({
                supply: 1,
                recipient: AssetTransferAddress.fromTypeAndPayload(
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

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
        });

        it("Can transfer trivially success script", async function() {
            const Opcode = Script.Opcode;
            const triviallySuccess = Buffer.from([Opcode.PUSH, 1]);
            const { asset } = await node.mintAsset({
                supply: 1,
                recipient: AssetTransferAddress.fromTypeAndPayload(
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

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.true;
        });

        it("Cannot transfer when lock script left multiple values in stack", async function() {
            const Opcode = Script.Opcode;
            const leaveMultipleValue = Buffer.from([
                Opcode.PUSH,
                1,
                Opcode.DUP
            ]);
            const { asset } = await node.mintAsset({
                supply: 1,
                recipient: AssetTransferAddress.fromTypeAndPayload(
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

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
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
            const invoice = await node
                .sendTransaction(transferTx, {
                    account: approver
                })
                .then(hash => {
                    return node.sdk.rpc.chain.getInvoice(hash, {
                        timeout: 300 * 1000
                    });
                });
            if (invoice == null) {
                throw Error("Cannot get the invoice");
            }
            expect(invoice).to.be.true;
        });

        it("nonApprover sends a transaction", async function() {
            const invoice = await node
                .sendTransaction(transferTx, {
                    account: nonApprover
                })
                .then(hash => {
                    return node.sdk.rpc.chain.getInvoice(hash, {
                        timeout: 300 * 1000
                    });
                });
            if (invoice == null) {
                throw Error("Cannot get the invoice");
            }
            expect(invoice).to.be.false;
        });
    });

    describe("Partial signature", function() {
        let assets: Asset[];
        let assetType: H256;
        let address1: AssetTransferAddress;
        let address2: AssetTransferAddress;
        let burnAddress1: AssetTransferAddress;
        let burnAddress2: AssetTransferAddress;
        beforeEach(async function() {
            address1 = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            address2 = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            burnAddress1 = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKHBurn"
            });
            burnAddress2 = await node.sdk.key.createAssetTransferAddress({
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.true;
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.true;
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;
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
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.true;
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
            const address1Param = (tx as any)._transaction.outputs[0]
                .parameters;
            const address2Param = (tx as any)._transaction.outputs[1]
                .parameters;
            ((tx as any)._transaction.outputs[0]
                .parameters as any) = address2Param;
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0]).to.be.false;

            ((tx as any)._transaction.outputs[0]
                .parameters as any) = address1Param;
            // FIXME
            (tx as any)._fee = null;
            (tx as any)._seq = null;
            await node.sdk.key.signTransactionInput(tx, 0, {
                signatureTag: {
                    input: "all",
                    output: [0]
                }
            });

            ((tx as any)._transaction.outputs[1]
                .parameters as any) = address1Param;
            const invoices2 = await node.sendAssetTransaction(tx);
            expect(invoices2!.length).to.equal(1);
            expect(invoices2![0]).to.be.true;
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
                    const invoices = await node.sendAssetTransaction(tx);
                    expect(invoices!.length).to.equal(1);
                    expect(invoices![0]).to.be.true;
                }).timeout(length * 10 + 5_000);
            });
        });
    });

    describe("Wrap CCC", function() {
        [1, 100].forEach(function(amount) {
            it(`Wrap successful - quantity {amount}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const transaction = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        quantity: amount,
                        payer: PlatformAddress.fromAccountId(faucetAccointId, {
                            networkId: "tc"
                        })
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                    transaction
                );
                const invoice = await node.sdk.rpc.chain.getInvoice(hash, {
                    timeout: 120 * 1000
                });
                expect(invoice).to.be.true;
            });
        });

        it("Wrap unsuccessful - quantity 0", async function() {
            const recipient = await node.createP2PKHAddress();
            const transaction = node.sdk.core
                .createWrapCCCTransaction({
                    shardId: 0,
                    recipient,
                    quantity: 0,
                    payer: PlatformAddress.fromAccountId(faucetAccointId, {
                        networkId: "tc"
                    })
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            try {
                await node.sdk.rpc.chain.sendSignedTransaction(transaction);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_ZERO_QUANTITY);
            }
        });
    });

    describe("Unwrap CCC", function() {
        describe("Wrap CCC with P2PKHBurnAddress", function() {
            let recipient: AssetTransferAddress;
            let wrapTransaction: SignedTransaction;
            let quantity: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHBurnAddress();
                wrapTransaction = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        quantity,
                        payer: PlatformAddress.fromAccountId(faucetAccointId, {
                            networkId: "tc"
                        })
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                    wrapTransaction
                );
                const invoice = await node.sdk.rpc.chain.getInvoice(hash, {
                    timeout: 120 * 1000
                });
                expect(invoice).to.be.true;
            });

            it("Unwrap successful", async function() {
                const tx = node.sdk.core.createUnwrapCCCTransaction({
                    burn: wrapTransaction.getAsset()
                });
                await node.signTransactionBurn(tx, 0);
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });
        });

        describe("Wrap CCC with P2PKHAddress", function() {
            let recipient: AssetTransferAddress;
            let wrapTransaction: SignedTransaction;
            let quantity: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHAddress();
                wrapTransaction = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        quantity,
                        payer: PlatformAddress.fromAccountId(faucetAccointId, {
                            networkId: "tc"
                        })
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                    wrapTransaction
                );
                const invoice = await node.sdk.rpc.chain.getInvoice(hash, {
                    timeout: 120 * 1000
                });
                expect(invoice).to.be.true;
            });

            it("Transfer then Unwrap successful", async function() {
                const recipientBurn = await node.createP2PKHBurnAddress();
                const asset1 = wrapTransaction.getAsset();

                const transferTx = node.sdk.core.createTransferAssetTransaction();
                transferTx.addInputs(asset1);
                transferTx.addOutputs({
                    assetType: asset1.assetType,
                    shardId: asset1.shardId,
                    recipient: recipientBurn,
                    quantity
                });
                await node.signTransactionInput(transferTx, 0);
                const invoices1 = await node.sendAssetTransaction(transferTx);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0]).to.be.true;

                const asset2 = await node.sdk.rpc.chain.getAsset(
                    transferTx.tracker(),
                    0,
                    asset1.shardId
                );

                const unwrapTx = node.sdk.core.createUnwrapCCCTransaction({
                    burn: asset2!
                });
                await node.signTransactionBurn(unwrapTx, 0);
                const invoices2 = await node.sendAssetTransaction(unwrapTx);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0]).to.be.true;
            });
        });

        describe("With minted asset (not wrapped CCC)", function() {
            let recipient: AssetTransferAddress;
            let mintTx: MintAsset;
            const supply: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHBurnAddress();
                const scheme = node.sdk.core.createAssetScheme({
                    shardId: 0,
                    metadata: "",
                    supply
                });
                mintTx = node.sdk.core.createMintAssetTransaction({
                    scheme,
                    recipient
                });
                const invoices = await node.sendAssetTransaction(mintTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });

            it("Unwrap unsuccessful - Invalid asset type", async function() {
                const tx = node.sdk.core.createUnwrapCCCTransaction({
                    burn: mintTx.getMintedAsset()
                });
                await node.signTransactionBurn(tx, 0);
                try {
                    await node.sendAssetTransaction(tx);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.INVALID_TX_ASSET_TYPE);
                }
            });
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
