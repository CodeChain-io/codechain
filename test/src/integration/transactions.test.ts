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

import * as _ from "lodash";
import {
    Asset,
    AssetTransferTransaction,
    PlatformAddress,
    AssetTransferAddress,
    SignedParcel,
    AssetMintTransaction
} from "codechain-sdk/lib/core/classes";

import CodeChain from "../helper/spawn";
import { faucetAddress, faucetSecret } from "../helper/constants";

describe("transactions", () => {
    let node: CodeChain;
    beforeAll(async () => {
        node = new CodeChain();
        await node.start();
    });

    describe("AssetMint", async () => {
        test.each([[1], [100]])("Mint successful - amount %i", async amount => {
            const recipient = await node.createP2PKHAddress();
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                amount
            });
            const tx = node.sdk.core.createAssetMintTransaction({
                scheme,
                recipient
            });
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(true);
        });

        test("Mint unsuccessful - mint amount 0", async () => {
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                amount: 0
            });
            const tx = node.sdk.core.createAssetMintTransaction({
                scheme,
                recipient: await node.createP2PKHAddress()
            });
            await expect(node.sendTransaction(tx)).rejects.toMatchObject({
                data: expect.stringContaining("ZeroAmount")
            });
        });

        test.skip("mint amount U64 max", done => done.fail("not implemented"));
        test.skip("mint amount exceeds U64", done =>
            done.fail("not implemented"));
    });

    describe("AssetTransfer - 1 input (100 amount)", async () => {
        let input: Asset;
        const amount = 100;

        beforeEach(async () => {
            const { asset } = await node.mintAsset({ amount });
            input = asset;
        });

        test.each([[[100]], [[99, 1]], [[1, 99]], [Array(100).fill(1)]])(
            "Transfer successful - output amount list: %p",
            async amounts => {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createAssetTransferTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    amounts.map(amount => ({
                        assetType: input.assetType,
                        recipient,
                        amount
                    }))
                );
                await node.signTransferInput(tx, 0);
                const invoices = await node.sendTransaction(tx);
                expect(invoices!.length).toBe(1);
                expect(invoices![0].success).toBe(true);
            }
        );

        test.each([[[0]], [[99]], [[101]], [[100, 100]]])(
            "Transfer unsuccessful(InconsistentTransactionInOut) - output amount list: %p",
            async amounts => {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createAssetTransferTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    amounts.map(amount => ({
                        assetType: input.assetType,
                        recipient,
                        amount
                    }))
                );
                await node.signTransferInput(tx, 0);
                await expect(node.sendTransaction(tx)).rejects.toMatchObject({
                    data: expect.stringContaining(
                        "InconsistentTransactionInOut"
                    )
                });
            }
        );
        test("Transfer unsuccessful(ZeroAmount) - output amount list: [100, 0]", async () => {
            const amounts = [100, 0];
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createAssetTransferTransaction();
            tx.addInputs(input);
            tx.addOutputs(
                amounts.map(amount => ({
                    assetType: input.assetType,
                    recipient,
                    amount
                }))
            );
            await node.signTransferInput(tx, 0);
            await expect(node.sendTransaction(tx)).rejects.toMatchObject({
                data: expect.stringContaining("ZeroAmount")
            });
        });

        test("wrong asset type", async () => {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createAssetTransferTransaction();
            tx.addInputs(input);
            tx.addOutputs({
                assetType:
                    "0x0000000000000000000000000000000000000000000000000000000000000000",
                recipient,
                amount
            });
            await node.signTransferInput(tx, 0);
            await expect(node.sendTransaction(tx)).rejects.toMatchObject({
                data: expect.stringContaining("InconsistentTransactionInOut")
            });
        });
    });

    describe("AssetTransfer - 2 different types of input (10 amount, 20 amount)", async () => {
        let input1: Asset;
        let input2: Asset;
        const amount1 = 10;
        const amount2 = 20;

        beforeEach(async () => {
            let { asset } = await node.mintAsset({ amount: amount1 });
            input1 = asset;
            ({ asset } = await node.mintAsset({ amount: amount2 }));
            input2 = asset;
        });

        test.each([
            [[10], [20]],
            [[5, 5], [10, 10]],
            [[1, 1, 1, 1, 1, 5], [1, 1, 1, 1, 1, 5, 10]]
        ])(
            "Transfer successful - asset1 %p, asset2 %p",
            async (input1Amounts, input2Amounts) => {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createAssetTransferTransaction();
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
                await node.signTransferInput(tx, 0);
                await node.signTransferInput(tx, 1);
                const invoices = await node.sendTransaction(tx);
                expect(invoices!.length).toBe(1);
                expect(invoices![0].success).toBe(true);
            }
        );
    });

    test("Burn successful", async () => {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createAssetTransferTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransferInput(tx1, 0);
        const invoices1 = await node.sendTransaction(tx1);
        expect(invoices1!.length).toBe(1);
        expect(invoices1![0].success).toBe(true);

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createAssetTransferTransaction();
        tx2.addBurns(transferredAsset);
        await node.signTransferBurn(tx2, 0);
        const invoices2 = await node.sendTransaction(tx2);
        expect(invoices2!.length).toBe(1);
        expect(invoices2![0].success).toBe(true);

        expect(await node.sdk.rpc.chain.getAsset(tx2.hash(), 0)).toBe(null);
    });

    test("Burn unsuccessful(ZeroAmount)", async () => {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createAssetTransferTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransferInput(tx1, 0);
        const invoices = await node.sendTransaction(tx1);
        expect(invoices!.length).toBe(1);
        expect(invoices![0].success).toBe(true);

        const tx2 = node.sdk.core.createAssetTransferTransaction();
        const {
            assetType,
            lockScriptHash,
            parameters
        } = tx1.getTransferredAsset(0);
        tx2.addBurns(
            node.sdk.core.createAssetTransferInput({
                assetOutPoint: {
                    assetType,
                    transactionHash: tx1.hash(),
                    index: 0,
                    lockScriptHash,
                    parameters,
                    amount: 0
                }
            })
        );
        await node.signTransferBurn(tx2, 0);
        await expect(node.sendTransaction(tx2)).rejects.toMatchObject({
            data: expect.stringContaining("ZeroAmount")
        });
    });

    test("Cannot transfer P2PKHBurn asset", async () => {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx1 = node.sdk.core.createAssetTransferTransaction();
        tx1.addInputs(asset);
        tx1.addOutputs({
            assetType: asset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransferInput(tx1, 0);
        const invoices1 = await node.sendTransaction(tx1);
        expect(invoices1!.length).toBe(1);
        expect(invoices1![0].success).toBe(true);

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createAssetTransferTransaction();
        tx2.addInputs(transferredAsset);
        tx2.addOutputs({
            assetType: transferredAsset.assetType,
            recipient: await node.createP2PKHBurnAddress(),
            amount: 1
        });
        await node.signTransactionP2PKHBurn(
            tx2.inputs[0],
            tx2.hashWithoutScript()
        );
        const invoices2 = await node.sendTransaction(tx2);
        expect(invoices2!.length).toBe(1);
        expect(invoices2![0].success).toBe(false);

        expect(await node.sdk.rpc.chain.getAsset(tx1.hash(), 0)).not.toBe(null);
    });

    test("Cannot burn P2PKH asset", async () => {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addBurns(asset);
        await node.signTransactionP2PKH(tx.burns[0], tx.hashWithoutScript());

        const invoices = await node.sendTransaction(tx);
        expect(invoices!.length).toBe(1);
        expect(invoices![0].success).toBe(false);
    });

    describe("registrar", () => {
        let registrar: PlatformAddress;
        let nonRegistrar: PlatformAddress;
        let transferTx: AssetTransferTransaction;
        beforeAll(async () => {
            registrar = await node.createPlatformAddress();
            nonRegistrar = await node.createPlatformAddress();
            await node.payment(registrar, 10000);
            await node.payment(nonRegistrar, 10000);
        });

        beforeEach(async () => {
            const recipient = await node.createP2PKHAddress();
            const tx = node.sdk.core.createAssetMintTransaction({
                scheme: {
                    shardId: 0,
                    metadata: "",
                    amount: 10000,
                    registrar
                },
                recipient
            });
            await node.sendTransaction(tx);
            const asset = await node.sdk.rpc.chain.getAsset(tx.hash(), 0);
            if (asset === null) {
                throw Error(`Failed to mint an asset`);
            }
            transferTx = node.sdk.core.createAssetTransferTransaction();
            transferTx.addInputs(asset);
            transferTx.addOutputs({
                assetType: asset.assetType,
                amount: 10000,
                recipient: await node.createP2PKHAddress()
            });
            await node.signTransferInput(transferTx, 0);
        });

        test("registrar sends a parcel", async () => {
            const invoice = await node
                .sendParcel(
                    node.sdk.core.createAssetTransactionParcel({
                        transaction: transferTx
                    }),
                    {
                        account: registrar
                    }
                )
                .then(hash => {
                    return node.sdk.rpc.chain.getParcelInvoice(hash, {
                        timeout: 300 * 1000
                    });
                });
            if (invoice == null) {
                throw Error("Cannot get the invoice");
            }
            expect(invoice.success).toBe(true);
        });

        test("nonRegistrar sends a parcel", async () => {
            const invoice = await node
                .sendParcel(
                    node.sdk.core.createAssetTransactionParcel({
                        transaction: transferTx
                    }),
                    {
                        account: nonRegistrar
                    }
                )
                .then(hash => {
                    return node.sdk.rpc.chain.getParcelInvoice(hash, {
                        timeout: 300 * 1000
                    });
                });
            if (invoice == null) {
                throw Error("Cannot get the invoice");
            }
            expect(invoice.success).toBe(false);
            expect(invoice.error!.type).toBe("InvalidTransaction");
            expect(invoice.error.content.type).toBe("NotRegistrar");
        });
    });

    describe("Partial signature", () => {
        let assets, assetType;
        let address1, address2, burnAddress1, burnAddress2;
        beforeEach(async () => {
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
            const mintTx = node.sdk.core.createAssetMintTransaction({
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
                .createAssetTransferTransaction()
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
            await node.sendTransaction(mintTx);
            await node.sendTransaction(transferTx);
        });

        test("Can't add burns after signing with the signature tag of all inputs", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 1000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addBurns(assets[2]);
            await node.sdk.key.signTransactionBurn(tx, 0);
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(false);
            expect(invoices![0].error!.type).toBe("InvalidTransaction");
            expect(invoices![0].error!.content.type).toBe("FailedToUnlock");
        });

        test("Can add burns after signing with the signature tag of single input", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
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
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(true);
        });

        // FIXME: (WIP) It fails
        test("Can't add inputs after signing with the signature tag of all inputs", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 2000,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addInputs(assets[1]);
            await node.sdk.key.signTransactionInput(tx, 1);
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(false);
            expect(invoices![0].error!.type).toBe("InvalidTransaction");
            expect(invoices![0].error!.content.type).toBe("FailedToUnlock");
        });

        test("Can add inputs after signing with the signature tag of single input", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
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
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(true);
        });

        test("Can't add outputs after signing the signature tag of all outputs", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
                .addInputs(assets[0])
                .addOutputs({
                    assetType,
                    amount: 500,
                    recipient: address1
                });
            await node.sdk.key.signTransactionInput(tx, 0);
            tx.addOutputs({ assetType, amount: 500, recipient: address2 });
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(false);
            expect(invoices![0].error!.type).toBe("InvalidTransaction");
            expect(invoices![0].error!.content.type).toBe("FailedToUnlock");
        });

        test("Can add outputs after signing the signature tag of some outputs", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
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
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(true);
        });

        test("Can only change the output protected by signature", async () => {
            const tx = node.sdk.core
                .createAssetTransferTransaction()
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
            const address1Param = tx.outputs[0].parameters;
            const address2Param = tx.outputs[1].parameters;
            (tx.outputs[0].parameters as any) = address2Param;
            const invoices = await node.sendTransaction(tx);
            expect(invoices!.length).toBe(1);
            expect(invoices![0].success).toBe(false);

            (tx.outputs[0].parameters as any) = address1Param;
            await node.sdk.key.signTransactionInput(tx, 0, {
                signatureTag: {
                    input: "all",
                    output: [0]
                }
            });

            (tx.outputs[1].parameters as any) = address1Param;
            const invoices2 = await node.sendTransaction(tx);
            expect(invoices2!.length).toBe(1);
            expect(invoices2![0].success).toBe(true);
        });

        describe("many outputs", () => {
            test.each([[5], [10], [100], [504]])(
                "%p + 1 outputs",
                async length => {
                    jest.setTimeout(length * 10 + 5000);
                    const tx = node.sdk.core
                        .createAssetTransferTransaction()
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
                    const invoices = await node.sendTransaction(tx);
                    expect(invoices!.length).toBe(1);
                    expect(invoices![0].success).toBe(true);
                }
            );
        });
    });

    describe("Asset compose and decompose test", () => {
        test("AssetCompose", async () => {
            const aliceAddress = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            const assetScheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: JSON.stringify({
                    name: "An example asset"
                }),
                amount: 10,
                registrar: null
            });
            const mintTx = node.sdk.core.createAssetMintTransaction({
                scheme: assetScheme,
                recipient: aliceAddress
            });
            const firstAsset = mintTx.getMintedAsset();
            const composeTx = node.sdk.core.createAssetComposeTransaction({
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
            const parcel0 = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: mintTx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            await node.sdk.rpc.chain.sendSignedParcel(parcel0);

            const parcel1 = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: composeTx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: seq + 1
                });
            await node.sdk.rpc.chain.sendSignedParcel(parcel1);

            const invoice0 = await node.sdk.rpc.chain.getParcelInvoice(
                parcel0.hash(),
                {
                    timeout: 300 * 1000
                }
            );
            if (invoice0 == null) {
                throw Error("Cannot get the invoice of mint transaction");
            }
            expect(invoice0.success).toBe(true);

            const invoice1 = await node.sdk.rpc.chain.getParcelInvoice(
                parcel1.hash(),
                {
                    timeout: 300 * 1000
                }
            );
            if (invoice1 == null) {
                throw Error("Cannot get the invoice of compose transaction");
            }
            expect(invoice1.success).toBe(true);
        });

        test("AssetDecompose", async () => {
            const aliceAddress = await node.sdk.key.createAssetTransferAddress({
                type: "P2PKH"
            });
            const assetScheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: JSON.stringify({
                    name: "An example asset"
                }),
                amount: 10,
                registrar: null
            });
            const mintTx = node.sdk.core.createAssetMintTransaction({
                scheme: assetScheme,
                recipient: aliceAddress
            });
            const firstAsset = mintTx.getMintedAsset();
            const composeTx = node.sdk.core.createAssetComposeTransaction({
                scheme: {
                    shardId: 0,
                    metadata: JSON.stringify({ name: "An unique asset" }),
                    amount: 1
                },
                inputs: [firstAsset.createTransferInput()],
                recipient: aliceAddress
            });
            await node.sdk.key.signTransactionInput(composeTx, 0);

            const decomposeTx = node.sdk.core.createAssetDecomposeTransaction({
                input: composeTx.getComposedAsset().createTransferInput()
            });
            decomposeTx.addOutputs({
                amount: 10,
                assetType: firstAsset.assetType,
                recipient: aliceAddress
            });
            await node.sdk.key.signTransactionInput(decomposeTx, 0);

            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);

            const parcel0 = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: mintTx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            const parcel1 = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: composeTx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: seq + 1
                });
            const parcel2 = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: decomposeTx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: seq + 2
                });

            await node.sdk.rpc.chain.sendSignedParcel(parcel0);
            await node.sdk.rpc.chain.sendSignedParcel(parcel1);
            await node.sdk.rpc.chain.sendSignedParcel(parcel2);

            const invoice0 = await node.sdk.rpc.chain.getParcelInvoice(
                parcel0.hash(),
                {
                    timeout: 300 * 1000
                }
            );
            if (invoice0 == null) {
                throw Error("Cannot get the invoice of mint");
            }
            expect(invoice0.success).toBe(true);
            const invoice1 = await node.sdk.rpc.chain.getParcelInvoice(
                parcel1.hash(),
                {
                    timeout: 300 * 1000
                }
            );
            if (invoice1 == null) {
                throw Error("Cannot get the invoice of compose");
            }
            expect(invoice1.success).toBe(true);
            const invoice2 = await node.sdk.rpc.chain.getParcelInvoice(
                parcel2.hash(),
                {
                    timeout: 300 * 1000
                }
            );
            if (invoice2 == null) {
                throw Error("Cannot get the invoice of decompose");
            }
            expect(invoice2.success).toBe(true);
        });
    });

    describe("Wrap CCC", () => {
        test.each([[1], [100]])("Wrap successful - amount %i", async amount => {
            const recipient = await node.createP2PKHAddress();
            const parcel = node.sdk.core
                .createWrapCCCParcel({
                    shardId: 0,
                    recipient,
                    amount
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            const hash = await node.sdk.rpc.chain.sendSignedParcel(parcel);
            const invoice = await node.sdk.rpc.chain.getParcelInvoice(hash, {
                timeout: 120 * 1000
            });
            expect(invoice.success).toBe(true);
        });

        test("Wrap unsuccessful - amount 0", async () => {
            const recipient = await node.createP2PKHAddress();
            const parcel = node.sdk.core
                .createWrapCCCParcel({
                    shardId: 0,
                    recipient,
                    amount: 0
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            await expect(
                node.sdk.rpc.chain.sendSignedParcel(parcel)
            ).rejects.toMatchObject({
                code: -32099,
                data: expect.stringContaining("ZeroAmount")
            });
        });
    });

    describe("Unwrap CCC", () => {
        describe("Wrap CCC with P2PKHBurnAddress", () => {
            let recipient: AssetTransferAddress;
            let wrapParcel: SignedParcel;
            let amount: number = 100;
            beforeEach(async () => {
                recipient = await node.createP2PKHBurnAddress();
                wrapParcel = node.sdk.core
                    .createWrapCCCParcel({
                        shardId: 0,
                        recipient,
                        amount
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedParcel(
                    wrapParcel
                );
                const invoice = await node.sdk.rpc.chain.getParcelInvoice(
                    hash,
                    {
                        timeout: 120 * 1000
                    }
                );
                expect(invoice.success).toBe(true);
            });

            test("Unwrap successful", async () => {
                const tx = node.sdk.core.createAssetUnwrapCCCTransaction({
                    burn: wrapParcel.getAsset()
                });
                await node.signTransferBurn(tx, 0);
                const invoices = await node.sendTransaction(tx);
                expect(invoices!.length).toBe(1);
                expect(invoices![0].success).toBe(true);
            });
        });

        describe("Wrap CCC with P2PKHAddress", () => {
            let recipient: AssetTransferAddress;
            let wrapParcel: SignedParcel;
            let amount: number = 100;
            beforeEach(async () => {
                recipient = await node.createP2PKHAddress();
                wrapParcel = node.sdk.core
                    .createWrapCCCParcel({
                        shardId: 0,
                        recipient,
                        amount
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    });

                const hash = await node.sdk.rpc.chain.sendSignedParcel(
                    wrapParcel
                );
                const invoice = await node.sdk.rpc.chain.getParcelInvoice(
                    hash,
                    {
                        timeout: 120 * 1000
                    }
                );
                expect(invoice.success).toBe(true);
            });

            test("Transfer then Unwrap successful", async () => {
                const recipientBurn = await node.createP2PKHBurnAddress();
                const asset1 = wrapParcel.getAsset();

                const transferTx = node.sdk.core.createAssetTransferTransaction();
                transferTx.addInputs(asset1);
                transferTx.addOutputs({
                    assetType: asset1.assetType,
                    recipient: recipientBurn,
                    amount
                });
                await node.signTransferInput(transferTx, 0);
                const invoices1 = await node.sendTransaction(transferTx);
                expect(invoices1!.length).toBe(1);
                expect(invoices1![0].success).toBe(true);

                const asset2 = await node.sdk.rpc.chain.getAsset(
                    transferTx.hash(),
                    0
                );

                const unwrapTx = node.sdk.core.createAssetUnwrapCCCTransaction({
                    burn: asset2
                });
                await node.signTransferBurn(unwrapTx, 0);
                const invoices2 = await node.sendTransaction(unwrapTx);
                expect(invoices2!.length).toBe(1);
                expect(invoices2![0].success).toBe(true);
            });
        });

        describe("With minted asset (not wrapped CCC)", () => {
            let recipient: AssetTransferAddress;
            let mintTx: AssetMintTransaction;
            let amount: number = 100;
            beforeEach(async () => {
                recipient = await node.createP2PKHBurnAddress();
                const scheme = node.sdk.core.createAssetScheme({
                    shardId: 0,
                    metadata: "",
                    amount
                });
                mintTx = node.sdk.core.createAssetMintTransaction({
                    scheme,
                    recipient
                });
                const invoices = await node.sendTransaction(mintTx);
                expect(invoices!.length).toBe(1);
                expect(invoices![0].success).toBe(true);
            });

            test("Unwrap unsuccessful - Invalid asset type", async () => {
                const tx = node.sdk.core.createAssetUnwrapCCCTransaction({
                    burn: mintTx.getMintedAsset()
                });
                await node.signTransferBurn(tx, 0);
                await expect(node.sendTransaction(tx)).rejects.toMatchObject({
                    code: -32099,
                    data: expect.stringContaining("InvalidAssetType")
                });
            });
        });
    });

    afterAll(async () => {
        await node.clean();
    });
});
