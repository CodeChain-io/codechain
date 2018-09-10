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
    AssetTransferTransaction
} from "codechain-sdk/lib/core/classes";
import { PlatformAddress } from "codechain-sdk/lib/key/classes";

import CodeChain from "../helper/spawn";

describe("transactions", () => {
    let node: CodeChain;
    beforeAll(async () => {
        node = new CodeChain();
        await node.start();
    });

    describe("AssetMint", async () => {
        test.each([[1], [100]])("mint amount %i", async amount => {
            const recipient = await node.createP2PKHAddress();
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                worldId: 0,
                metadata: "",
                amount
            });
            const tx = node.sdk.core.createAssetMintTransaction({
                scheme,
                recipient
            });
            const invoice = await node.sendTransaction(tx);
            expect(invoice.success).toBe(true);
        });

        test.skip("mint amount 0", done => done.fail("not implemented"));
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
                    ...amounts.map(amount => ({
                        assetType: input.assetType,
                        recipient,
                        amount
                    }))
                );
                await node.signTransferInput(tx, 0);
                const invoice = await node.sendTransaction(tx);
                expect(invoice.success).toBe(true);
            }
        );

        test.each([[[0]], [[99]], [[101]], [[100, 100]]])(
            "Transfer unsuccessful - output amount list: %p",
            async amounts => {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createAssetTransferTransaction();
                tx.addInputs(input);
                tx.addOutputs(
                    ...amounts.map(amount => ({
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
        test.skip("Transfer unsuccessful - output amount list: [100, 0]", done =>
            done.fail());

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
                tx.addInputs(..._.shuffle([input1, input2]));
                tx.addOutputs(
                    ..._.shuffle([
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
                const invoice = await node.sendTransaction(tx);
                expect(invoice.success).toBe(true);
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

        const transferredAsset = tx1.getTransferredAsset(0);
        const tx2 = node.sdk.core.createAssetTransferTransaction();
        tx2.addBurns(transferredAsset);
        await node.signTransferBurn(tx2, 0);

        const invoices = await node.sendTransactions([tx1, tx2]);

        expect(invoices[0].success).toBe(true);
        expect(invoices[1].success).toBe(true);

        expect(await node.sdk.rpc.chain.getAsset(tx2.hash(), 0)).toBe(null);
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
                    worldId: 0,
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
                    node.sdk.core.createAssetTransactionGroupParcel({
                        transactions: [transferTx]
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
            expect(invoice[0].success).toBe(true);
        });

        test("nonRegistrar sends a parcel", async () => {
            const invoice = await node
                .sendParcel(
                    node.sdk.core.createAssetTransactionGroupParcel({
                        transactions: [transferTx]
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
            expect(invoice[0].success).toBe(false);
            expect(invoice[0].error.type).toContain("NotRegistrar");
        });
    });

    test.skip("CreateWorld", done => done.fail("not implemented"));
    test.skip("SetWorldOwners", done => done.fail("not implemented"));
    test.skip("SetWorldUsers", done => done.fail("not implemented"));

    afterAll(async () => {
        await node.clean();
    });
});
