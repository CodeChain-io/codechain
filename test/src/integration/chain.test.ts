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

import { SDK } from "codechain-sdk";
import { PlatformAddress } from "codechain-sdk/lib/key/PlatformAddress";

import {
    H256,
    H512,
    U256,
    AssetMintTransaction,
    AssetScheme
} from "codechain-sdk/lib/core/classes";

import CodeChain from "../helper/spawn";

describe("solo - 1 node", () => {
    const secret =
        "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
    const address = PlatformAddress.fromAccountId(
        SDK.util.getAccountIdFromPrivate(secret)
    );
    const noSuchAddress = "tccqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqj5aqu5";
    const invalidHash = new H256("0".repeat(64));

    let node: CodeChain;
    beforeAll(async () => {
        node = new CodeChain();
        await node.start();
    });

    test("getNetworkId", async () => {
        expect(await node.sdk.rpc.chain.getNetworkId()).toBe("tc");
    });

    test("getBestBlockNumber", async () => {
        expect(typeof (await node.sdk.rpc.chain.getBestBlockNumber())).toEqual(
            "number"
        );
    });

    test("getBestBlockId", async () => {
        const value = await node.sdk.rpc.sendRpcRequest(
            "chain_getBestBlockId",
            []
        );
        expect(typeof value.hash).toEqual("string");
        new H256(value.hash);
        expect(typeof value.number).toEqual("number");
    });

    test("getBlockHash", async () => {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(
            await node.sdk.rpc.chain.getBlockHash(bestBlockNumber)
        ).not.toBeNull();
        expect(
            await node.sdk.rpc.chain.getBlockHash(bestBlockNumber + 1)
        ).toBeNull();
    });

    test("getBlockByHash", async () => {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const blockHash = await node.sdk.rpc.chain.getBlockHash(
            bestBlockNumber
        );
        expect((await node.sdk.rpc.chain.getBlock(blockHash)).number).toEqual(
            bestBlockNumber
        );
        expect(await node.sdk.rpc.chain.getBlock(invalidHash)).toBeNull();
    });

    test("getNonce", async () => {
        // TODO: invalid address
        await node.sdk.rpc.chain.getNonce(address);
        expect(await node.sdk.rpc.chain.getNonce(noSuchAddress)).toEqual(
            new U256(0)
        );
    });

    test("getBalance", async () => {
        // TODO: invalid address
        await node.sdk.rpc.chain.getBalance(address);
        expect(await node.sdk.rpc.chain.getBalance(noSuchAddress)).toEqual(
            new U256(0)
        );
    });

    test("getCoinbase", async () => {
        // TODO: Coinbase is not defined in solo mode, so it always returns null. Need to test in other modes.
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getCoinbase", [])
        ).toBeNull();
    });

    test("getPendingParcels", async () => {
        const pendingParcels = await node.sdk.rpc.chain.getPendingParcels();
        expect(pendingParcels.length).toEqual(0);
    });

    test("sendSignedParcel, getParcelInvoice, getParcel", async () => {
        const parcel = node.sdk.core.createPaymentParcel({
            recipient: "tccqruq09sfgax77nj4gukjcuq69uzeyv0jcs7vzngg",
            amount: 0
        });
        const nonce = await node.sdk.rpc.chain.getNonce(address);
        const parcelHash = await node.sdk.rpc.chain.sendSignedParcel(
            parcel.sign({
                secret,
                fee: 10,
                nonce
            })
        );
        const invoice = await node.sdk.rpc.chain.getParcelInvoice(parcelHash);
        expect(invoice).toEqual({ success: true });
        const signedParcel = await node.sdk.rpc.chain.getParcel(parcelHash);
        expect(signedParcel.unsigned).toEqual(parcel);
    });

    test("getRegularKey, getRegularKeyOwner", async () => {
        const key = node.sdk.util.getPublicFromPrivate(
            node.sdk.util.generatePrivateKey()
        );
        expect(await node.sdk.rpc.chain.getRegularKey(address)).toBeNull();
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key)).toBeNull();

        const parcel = node.sdk.core
            .createSetRegularKeyParcel({
                key
            })
            .sign({
                secret,
                fee: 10,
                nonce: await node.sdk.rpc.chain.getNonce(address)
            });
        await node.sdk.rpc.chain.sendSignedParcel(parcel);

        expect(await node.sdk.rpc.chain.getRegularKey(address)).toEqual(
            new H512(key)
        );
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key)).toEqual(
            address
        );
    });

    describe("Mint an asset", () => {
        let tx: AssetMintTransaction;
        let txAssetScheme: AssetScheme;

        beforeAll(async () => {
            const recipient = await node.createP2PKHAddress();
            tx = node.sdk.core.createAssetMintTransaction({
                scheme: {
                    shardId: 0,
                    worldId: 0,
                    metadata: "",
                    amount: 10
                },
                recipient
            });
            txAssetScheme = tx.getAssetScheme();

            const parcel = node.sdk.core
                .createAssetTransactionGroupParcel({
                    transactions: [tx]
                })
                .sign({
                    secret,
                    fee: 10,
                    nonce: await node.sdk.rpc.chain.getNonce(address)
                });

            await node.sdk.rpc.chain.sendSignedParcel(parcel);
        });

        test("getTransaction", async () => {
            expect(await node.sdk.rpc.chain.getTransaction(tx.hash())).toEqual(
                tx
            );
            expect(
                await node.sdk.rpc.chain.getTransaction(invalidHash)
            ).toBeNull();
        });

        test("getTransactionInvoice", async () => {
            expect(
                (await node.sdk.rpc.chain.getTransactionInvoice(tx.hash()))
                    .success
            ).toBe(true);
        });

        test("getAsset", async () => {
            expect(
                await node.sdk.rpc.chain.getAsset(invalidHash, 0)
            ).toBeNull();
            expect(await node.sdk.rpc.chain.getAsset(tx.hash(), 1)).toBeNull();

            const asset = await node.sdk.rpc.chain.getAsset(tx.hash(), 0);
            expect(asset).toEqual(tx.getMintedAsset());
        });

        test("getAssetSchemeByHash", async () => {
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByHash(invalidHash, 0, 0)
            ).toBeNull();
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByHash(tx.hash(), 1, 0)
            ).toBeNull();
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByHash(tx.hash(), 0, 1)
            ).toBeNull();

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByHash(
                tx.hash(),
                0,
                0
            );
            expect(assetScheme.amount).toEqual(txAssetScheme.amount);
            expect(assetScheme.metadata).toEqual(txAssetScheme.metadata);
            expect(assetScheme.registrar).toEqual(txAssetScheme.registrar);
        });

        test("getAssetSchemeByType", async () => {
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByType(invalidHash)
            ).toBeNull();

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByType(
                tx.getAssetSchemeAddress()
            );
            expect(assetScheme.amount).toEqual(txAssetScheme.amount);
            expect(assetScheme.metadata).toEqual(txAssetScheme.metadata);
            expect(assetScheme.registrar).toEqual(txAssetScheme.registrar);
        });
    });

    test("isAssetSpent", async () => {
        const { asset } = await node.mintAsset({ amount: 10 });
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0
            )
        ).toBe(false);

        const recipient = await node.createP2PKHAddress();
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(asset);
        tx.addOutputs({
            assetType: asset.assetType,
            recipient,
            amount: 10
        });
        await node.signTransferInput(tx, 0);
        expect((await node.sendTransaction(tx)).success).toBe(true);
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0
            )
        ).toBe(true);
    });

    test.skip("executeTransactions", done => done.fail("not implemented"));
    test.skip("getNumberOfShards", done => done.fail("not implemented"));
    test.skip("getShardRoot", done => done.fail("not implemented"));

    // TODO:
    // block number in
    // * getAsset
    // * isAssetSpent
    // * getNonce
    // * getBalance
    // * getRegularKey
    // * getRegularKeyOwner

    afterAll(async () => {
        await node.clean();
    });
});
