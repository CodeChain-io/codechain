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

import {
    Asset,
    AssetTransferAddress,
    H256,
    MintAsset,
    PlatformAddress,
    Script,
    SignedTransaction,
    TransferAsset,
    U64
} from "codechain-sdk/lib/core/classes";
import * as _ from "lodash";
import { Buffer } from "buffer";
import { P2PKH } from "codechain-sdk/lib/key/P2PKH";
import { blake160 } from "codechain-sdk/lib/utils";

import CodeChain from "../helper/spawn";
import { ERROR, errorMatcher } from "../helper/error";
import { faucetAddress, faucetSecret } from "../helper/constants";

import "mocha";
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;

describe("transactions", function() {
    let node: CodeChain;
    const BASE = 700;
    before(async function() {
        node = new CodeChain({ base: BASE });
        await node.start();
    });

    describe("AssetMint", async function() {
        [1, 100, U64.MAX_VALUE].forEach(function(amount) {
            it(`Mint successful - amount ${amount}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const scheme = node.sdk.core.createAssetScheme({
                    shardId: 0,
                    metadata: "",
                    amount
                });
                const tx = node.sdk.core.createMintAssetTransaction({
                    scheme,
                    recipient
                });
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });
        });

        it("Mint unsuccessful - mint amount 0", async function() {
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                amount: 0
            });
            const tx = node.sdk.core.createMintAssetTransaction({
                scheme,
                recipient: await node.createP2PKHAddress()
            });

            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.INVALID_TX_ZERO_AMOUNT)
                );
            }
        });

        it("Mint unsuccessful - mint amount U64.MAX_VALUE + 1", async function() {
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                amount: 0
            });
            (scheme.amount.value as any) = U64.MAX_VALUE.value.plus(1);

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
                expect(e).to.satisfy(errorMatcher(ERROR.INVALID_RLP_TOO_BIG));
            }
        });
    });

    describe("AssetTransfer - 1 input (100 amount)", async function() {
        let input: Asset;
        const amount = 100;

        beforeEach(async function() {
            const { asset } = await node.mintAsset({ amount });
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
                    amounts.map(amount => ({
                        assetType: input.assetType,
                        recipient,
                        amount
                    }))
                );
                await node.signTransactionInput(tx, 0);
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });
        });

        [[0], [99], [101], [100, 100]].forEach(function(amounts) {
            it(`Transfer unsuccessful(InconsistentTransactionInOut) - output amount list: ${amounts}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createTransferAssetTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    amounts.map(amount => ({
                        assetType: input.assetType,
                        recipient,
                        amount
                    }))
                );
                await node.signTransactionInput(tx, 0);
                try {
                    await node.sendAssetTransaction(tx);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_TX_INCONSISTENT_IN_OUT)
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
                amounts.map(amount => ({
                    assetType: input.assetType,
                    recipient,
                    amount
                }))
            );
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.INVALID_TX_ZERO_AMOUNT)
                );
            }
        });

        it("Transfer unsuccessful - wrong asset type", async function() {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs(input);
            tx.addOutputs({
                assetType:
                    "0x0000000000000000000000000000000000000000000000000000000000000000",
                recipient,
                amount
            });
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.INVALID_TX_INCONSISTENT_IN_OUT)
                );
            }
        });

        it("Transfer unsuccessful - previous output is duplicated", async function() {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs(input, input);
            tx.addOutputs({
                assetType: input.assetType,
                recipient,
                amount: amount * 2
            });
            await node.signTransactionInput(tx, 0);
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.INVALID_TX_DUPLICATED_PREV_OUT)
                );
            }
        });
    });

    describe("AssetTransfer - 2 different types of input (10 amount, 20 amount)", async function() {
        let input1: Asset;
        let input2: Asset;
        const amount1 = 10;
        const amount2 = 20;

        beforeEach(async function() {
            let { asset } = await node.mintAsset({ amount: amount1 });
            input1 = asset;
            ({ asset } = await node.mintAsset({ amount: amount2 }));
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
                            recipient,
                            amount
                        })),
                        ...input2Amounts.map(amount => ({
                            assetType: input2.assetType,
                            recipient,
                            amount
                        }))
                    ])
                );
                await node.signTransactionInput(tx, 0);
                await node.signTransactionInput(tx, 1);
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });
        });
    });

    it("Burn successful", async function() {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices1 = await node.sendAssetTransaction(tx1);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0].success).to.be.true;

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createTransferAssetTransaction();
        tx2.addBurns(transferredAsset);
        await node.signTransactionBurn(tx2, 0);
        const invoices2 = await node.sendAssetTransaction(tx2);
        expect(invoices2!.length).to.equal(1);
        expect(invoices2![0].success).to.be.true;

        expect(await node.sdk.rpc.chain.getAsset(tx2.tracker(), 0)).to.be.null;
    });

    it("Burn unsuccessful(ZeroAmount)", async function() {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices = await node.sendAssetTransaction(tx1);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0].success).to.be.true;

        const tx2 = node.sdk.core.createTransferAssetTransaction();
        const {
            assetType,
            lockScriptHash,
            parameters
        } = tx1.getTransferredAsset(0);
        tx2.addBurns(
            node.sdk.core.createAssetTransferInput({
                assetOutPoint: {
                    assetType,
                    tracker: tx1.tracker(),
                    index: 0,
                    lockScriptHash,
                    parameters,
                    amount: 0
                }
            })
        );
        await node.signTransactionBurn(tx2, 0);
        try {
            await node.sendAssetTransaction(tx2);
            expect.fail();
        } catch (e) {
            expect(e).to.satisfy(errorMatcher(ERROR.INVALID_TX_ZERO_AMOUNT));
        }
    });

    it("Cannot transfer P2PKHBurn asset", async function() {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createTransferAssetTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransactionInput(tx1, 0);
        const invoices1 = await node.sendAssetTransaction(tx1);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0].success).to.be.true;

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createTransferAssetTransaction();
        tx2.addInputs(transferredAsset);
        tx2.addOutputs({
            assetType: transferredAsset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransactionP2PKHBurn(
            tx2.input(0)!,
            tx2.hashWithoutScript()
        );
        const invoices2 = await node.sendAssetTransaction(tx2);
        expect(invoices2!.length).to.equal(1);
        expect(invoices2![0].success).to.be.false;
        expect(invoices2![0].error!.type).to.equal("InvalidTransaction");
        expect(invoices2![0].error!.content.type).to.equal("FailedToUnlock");
        expect(invoices2![0].error!.content.content.reason).to.be.equal(
            "ScriptShouldBeBurnt"
        );

        expect(await node.sdk.rpc.chain.getAsset(tx1.tracker(), 0)).not.to.be
            .null;
    });

    it("Cannot burn P2PKH asset", async function() {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addBurns(asset);
        await node.signTransactionP2PKH(tx.burn(0)!, tx.hashWithoutScript());

        const invoices = await node.sendAssetTransaction(tx);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0].success).to.be.false;
        expect(invoices![0].error!.type).to.equal("InvalidTransaction");
        expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
        expect(invoices![0].error!.content.content.reason).to.be.equal(
            "ScriptShouldNotBeBurnt"
        );
    });

    describe("ScriptError", function() {
        it("Cannot transfer with invalid unlock script", async function() {
            const Opcode = Script.Opcode;
            const { asset } = await node.mintAsset({ amount: 1 });
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    amount: 1,
                    assetType: asset.assetType,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(P2PKH.getLockScript());
            tx.input(0)!.setUnlockScript(Buffer.from([Opcode.NOP])); // Invalid Opcode for unlock_script
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.be.equal(
                "ScriptError"
            );
        });

        it("Cannot transfer trivially fail script", async function() {
            const triviallyFail = Buffer.from([0x03]); // Opcode.FAIL
            const { asset } = await node.mintAsset({
                amount: 1,
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
                    amount: 1,
                    assetType: asset.assetType,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(triviallyFail);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.be.equal(
                "ScriptError"
            );
        });

        it("Can transfer trivially success script", async function() {
            const Opcode = Script.Opcode;
            const triviallySuccess = Buffer.from([Opcode.PUSH, 1]);
            const { asset } = await node.mintAsset({
                amount: 1,
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
                    amount: 1,
                    assetType: asset.assetType,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(triviallySuccess);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.true;
        });

        it("Cannot transfer when lock script left multiple values in stack", async function() {
            const Opcode = Script.Opcode;
            const leaveMultipleValue = Buffer.from([
                Opcode.PUSH,
                1,
                Opcode.DUP
            ]);
            const { asset } = await node.mintAsset({
                amount: 1,
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
                    amount: 1,
                    assetType: asset.assetType,
                    recipient: await node.createP2PKHAddress()
                });
            tx.input(0)!.setLockScript(leaveMultipleValue);
            tx.input(0)!.setUnlockScript(Buffer.from([]));

            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.be.equal(
                "ScriptError"
            );
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
                    amount: 10000,
                    approver
                },
                recipient
            });
            await node.sendAssetTransaction(tx);
            const asset = await node.sdk.rpc.chain.getAsset(tx.tracker(), 0);
            if (asset === null) {
                throw Error(`Failed to mint an asset`);
            }
            transferTx = node.sdk.core.createTransferAssetTransaction();
            transferTx.addInputs(asset);
            transferTx.addOutputs({
                assetType: asset.assetType,
                amount: 10000,
                recipient: await node.createP2PKHAddress()
            });
            await node.signTransactionInput(transferTx, 0);
        });

        it("approver sends a parcel", async function() {
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
            expect(invoice.success).to.be.true;
        });

        it("nonApprover sends a parcel", async function() {
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
            expect(invoice.success).to.be.false;
            expect(invoice.error!.type).to.equal("InvalidTransaction");
            expect(invoice.error!.content.type).to.equal("NotApproved");
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
                    amount: 4000
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
                        amount: 1000,
                        recipient: address1
                    },
                    {
                        assetType,
                        amount: 1000,
                        recipient: address2
                    },
                    {
                        assetType,
                        amount: 1000,
                        recipient: burnAddress1
                    },
                    {
                        assetType,
                        amount: 1000,
                        recipient: burnAddress2
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
                    amount: 1000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addBurns(assets[2]);
            await node.sdk.key.signTransactionBurn(tx, 0);
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.equal(
                "ScriptError"
            );
        });

        it("Can add burns after signing with the signature tag of single input", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 1000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0, {
                signatureTag: { input: "single", output: "all" }
            });
            tx.addBurns(assets[2]);
            await node.sdk.key.signTransactionBurn(tx, 0);
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.true;
        });

        // FIXME: (WIP) It fails
        it("Can't add inputs after signing with the signature tag of all inputs", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 2000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addInputs(assets[1]);
            await node.sdk.key.signTransactionInput(tx, 1);
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.equal(
                "ScriptError"
            );
        });

        it("Can add inputs after signing with the signature tag of single input", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 2000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0, {
                signatureTag: { input: "single", output: "all" }
            });
            tx.addInputs(assets[1]);
            await node.sdk.key.signTransactionInput(tx, 1);
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.true;
        });

        it("Can't add outputs after signing the signature tag of all outputs", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 500,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addOutputs({ assetType, amount: 500, recipient: address2 });
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.false;
            expect(invoices![0].error!.type).to.equal("InvalidTransaction");
            expect(invoices![0].error!.content.type).to.equal("FailedToUnlock");
            expect(invoices![0].error!.content.content.reason).to.equal(
                "ScriptError"
            );
        });

        it("Can add outputs after signing the signature tag of some outputs", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 500,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0, {
                signatureTag: {
                    input: "all",
                    output: [0]
                }
            });
            tx.addOutputs({ assetType, amount: 500, recipient: address2 });
            const invoices = await node.sendAssetTransaction(tx);
            expect(invoices!.length).to.equal(1);
            expect(invoices![0].success).to.be.true;
        });

        it("Can only change the output protected by signature", async function() {
            const tx = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(assets[0])
                .addOutputs(
                    {
                        assetType,
                        amount: 500,
                        recipient: address1
                    },
                    {
                        assetType,
                        amount: 500,
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
            expect(invoices![0].success).to.be.false;

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
            expect(invoices2![0].success).to.be.true;
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
                                amount: 1,
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
                        amount: 1000 - length,
                        recipient: address1
                    });
                    const invoices = await node.sendAssetTransaction(tx);
                    expect(invoices!.length).to.equal(1);
                    expect(invoices![0].success).to.be.true;
                }).timeout(length * 10 + 5_000);
            });
        });
    });

    describe("Asset compose and decompose test", function() {
        it("AssetCompose", async function() {
            const aliceAddress = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            const assetScheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: JSON.stringify({
                    name: "An example asset"
                }),
                amount: 10
            });
            const mintTx = node.sdk.core.createMintAssetTransaction({
                scheme: assetScheme,
                recipient: aliceAddress
            });
            const firstAsset = mintTx.getMintedAsset();
            const composeTx = node.sdk.core.createComposeAssetTransaction({
                scheme: {
                    shardId: 0,
                    metadata: JSON.stringify({ name: "An unique asset" }),
                    amount: 1
                },
                inputs: [firstAsset.createTransferInput()],
                recipient: aliceAddress
            });
            await node.sdk.key.signTransactionInput(composeTx, 0);

            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const tx0 = mintTx.sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
            await node.sdk.rpc.chain.sendSignedTransaction(tx0);

            const tx1 = composeTx.sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 1
            });
            await node.sdk.rpc.chain.sendSignedTransaction(tx1);

            const invoice0 = await node.sdk.rpc.chain.getInvoice(tx0.hash(), {
                timeout: 300 * 1000
            });
            if (invoice0 == null) {
                throw Error("Cannot get the invoice of mint transaction");
            }
            expect(invoice0.success).to.be.true;

            const invoice1 = await node.sdk.rpc.chain.getInvoice(tx1.hash(), {
                timeout: 300 * 1000
            });
            if (invoice1 == null) {
                throw Error("Cannot get the invoice of compose transaction");
            }
            expect(invoice1.success).to.be.true;
        });

        it("AssetDecompose", async function() {
            const aliceAddress = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            const assetScheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: JSON.stringify({
                    name: "An example asset"
                }),
                amount: 10
            });
            const mintTx = node.sdk.core.createMintAssetTransaction({
                scheme: assetScheme,
                recipient: aliceAddress
            });
            const firstAsset = mintTx.getMintedAsset();
            const composeTx = node.sdk.core.createComposeAssetTransaction({
                scheme: {
                    shardId: 0,
                    metadata: JSON.stringify({ name: "An unique asset" }),
                    amount: 1
                },
                inputs: [firstAsset.createTransferInput()],
                recipient: aliceAddress
            });
            await node.sdk.key.signTransactionInput(composeTx, 0);

            const decomposeTx = node.sdk.core.createDecomposeAssetTransaction({
                input: composeTx.getComposedAsset().createTransferInput()
            });
            decomposeTx.addOutputs({
                amount: 10,
                assetType: firstAsset.assetType,
                recipient: aliceAddress
            });
            await node.sdk.key.signTransactionInput(decomposeTx, 0);

            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);

            const tx0 = mintTx.sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
            const tx1 = composeTx.sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 1
            });
            const tx2 = decomposeTx.sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 2
            });

            await node.sdk.rpc.chain.sendSignedTransaction(tx0);
            await node.sdk.rpc.chain.sendSignedTransaction(tx1);
            await node.sdk.rpc.chain.sendSignedTransaction(tx2);

            const invoice0 = await node.sdk.rpc.chain.getInvoice(tx0.hash(), {
                timeout: 300 * 1000
            });
            if (invoice0 == null) {
                throw Error("Cannot get the invoice of mint");
            }
            expect(invoice0.success).to.be.true;
            const invoice1 = await node.sdk.rpc.chain.getInvoice(tx1.hash(), {
                timeout: 300 * 1000
            });
            if (invoice1 == null) {
                throw Error("Cannot get the invoice of compose");
            }
            expect(invoice1.success).to.be.true;
            const invoice2 = await node.sdk.rpc.chain.getInvoice(tx2.hash(), {
                timeout: 300 * 1000
            });
            if (invoice2 == null) {
                throw Error("Cannot get the invoice of decompose");
            }
            expect(invoice2.success).to.be.true;
        });
    });

    describe("Wrap CCC", function() {
        [1, 100].forEach(function(amount) {
            it(`Wrap successful - amount {amount}`, async function() {
                const recipient = await node.createP2PKHAddress();
                const parcel = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        amount
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                    parcel
                );
                const invoice = await node.sdk.rpc.chain.getInvoice(hash, {
                    timeout: 120 * 1000
                });
                expect(invoice!.success).to.be.true;
            });
        });

        it("Wrap unsuccessful - amount 0", async function() {
            const recipient = await node.createP2PKHAddress();
            const parcel = node.sdk.core
                .createWrapCCCTransaction({
                    shardId: 0,
                    recipient,
                    amount: 0
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            try {
                await node.sdk.rpc.chain.sendSignedTransaction(parcel);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(
                    errorMatcher(ERROR.INVALID_PARCEL_ZERO_AMOUNT)
                );
            }
        });
    });

    describe("Unwrap CCC", function() {
        describe("Wrap CCC with P2PKHBurnAddress", function() {
            let recipient: AssetTransferAddress;
            let wrapTransaction: SignedTransaction;
            let amount: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHBurnAddress();
                wrapTransaction = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        amount
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
                expect(invoice!.success).to.be.true;
            });

            it("Unwrap successful", async function() {
                const tx = node.sdk.core.createUnwrapCCCTransaction({
                    burn: wrapTransaction.getAsset()
                });
                await node.signTransactionBurn(tx, 0);
                const invoices = await node.sendAssetTransaction(tx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });
        });

        describe("Wrap CCC with P2PKHAddress", function() {
            let recipient: AssetTransferAddress;
            let wrapTransaction: SignedTransaction;
            let amount: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHAddress();
                wrapTransaction = node.sdk.core
                    .createWrapCCCTransaction({
                        shardId: 0,
                        recipient,
                        amount
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
                expect(invoice!.success).to.be.true;
            });

            it("Transfer then Unwrap successful", async function() {
                const recipientBurn = await node.createP2PKHBurnAddress();
                const asset1 = wrapTransaction.getAsset();

                const transferTx = node.sdk.core.createTransferAssetTransaction();
                transferTx.addInputs(asset1);
                transferTx.addOutputs({
                    assetType: asset1.assetType,
                    recipient: recipientBurn,
                    amount
                });
                await node.signTransactionInput(transferTx, 0);
                const invoices1 = await node.sendAssetTransaction(transferTx);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0].success).to.be.true;

                const asset2 = await node.sdk.rpc.chain.getAsset(
                    transferTx.tracker(),
                    0
                );

                const unwrapTx = node.sdk.core.createUnwrapCCCTransaction({
                    burn: asset2!
                });
                await node.signTransactionBurn(unwrapTx, 0);
                const invoices2 = await node.sendAssetTransaction(unwrapTx);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0].success).to.be.true;
            });
        });

        describe("With minted asset (not wrapped CCC)", function() {
            let recipient: AssetTransferAddress;
            let mintTx: MintAsset;
            const amount: number = 100;
            beforeEach(async function() {
                recipient = await node.createP2PKHBurnAddress();
                const scheme = node.sdk.core.createAssetScheme({
                    shardId: 0,
                    metadata: "",
                    amount
                });
                mintTx = node.sdk.core.createMintAssetTransaction({
                    scheme,
                    recipient
                });
                const invoices = await node.sendAssetTransaction(mintTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
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
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_TX_ASSET_TYPE)
                    );
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
