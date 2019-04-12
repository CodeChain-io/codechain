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

import { Buffer } from "buffer";
import { expect } from "chai";
import {
    AssetAddress,
    H160,
    Script,
    TransferAsset,
    Transaction
} from "codechain-sdk/lib/core/classes";
import {
    blake160,
    encodeSignatureTag,
    signEcdsa
} from "codechain-sdk/lib/utils";
import "mocha";
import {
    alicePublic,
    aliceSecret,
    bobPublic,
    bobSecret,
    carolPublic,
    carolSecret,
    daveSecret,
    faucetAddress
} from "../helper/constants";
import { AssetTransaction } from "codechain-sdk/lib/core/Transaction";
import CodeChain from "../helper/spawn";

const { PUSH, PUSHB, CHKMULTISIG } = Script.Opcode;

// If one only sends certainly failing trasactions, the miner would not generate any block.
// So to clearly check the result failed, insert the failing transactions inbetween succeessful ones.
async function expectTransactionFail(
    node: CodeChain,
    targetTx: Transaction & AssetTransaction
) {
    await node.sdk.rpc.devel.stopSealing();

    const blockNumber = await node.getBestBlockNumber();
    const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
    const signedDummyTx = await node.sendPayTx({ seq, quantity: 1 });
    const targetTxHash = await node.sendAssetTransaction(targetTx, {
        seq: seq + 1
    });

    await node.sdk.rpc.devel.startSealing();
    await node.waitBlockNumber(blockNumber + 1);

    expect(await node.sdk.rpc.chain.containsTransaction(signedDummyTx.hash()))
        .be.true;
    expect(await node.sdk.rpc.chain.containsTransaction(targetTxHash)).be.false;
    expect(await node.sdk.rpc.chain.getErrorHint(targetTxHash)).not.null;
}

describe("Multisig", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("1 of 2", async function() {
        const lockScript = Buffer.from([
            PUSH,
            1,
            PUSHB,
            64,
            ...Buffer.from(alicePublic, "hex"),
            PUSHB,
            64,
            ...Buffer.from(bobPublic, "hex"),
            PUSH,
            2,
            CHKMULTISIG
        ]);
        const lockScriptHash = new H160(blake160(lockScript));

        let transfer: TransferAsset;
        beforeEach(async function() {
            const recipient = AssetAddress.fromTypeAndPayload(
                0,
                lockScriptHash,
                {
                    networkId: node.sdk.networkId
                }
            );

            const asset = await node.mintAsset({ supply: 1, recipient });

            transfer = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
        });

        it("unlock with the first key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, aliceSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, bobSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the unknown key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, carolSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });
    });

    describe("1 of 3", async function() {
        const lockScript = Buffer.from([
            PUSH,
            1,
            PUSHB,
            64,
            ...Buffer.from(alicePublic, "hex"),
            PUSHB,
            64,
            ...Buffer.from(bobPublic, "hex"),
            PUSHB,
            64,
            ...Buffer.from(carolPublic, "hex"),
            PUSH,
            3,
            CHKMULTISIG
        ]);
        const lockScriptHash = new H160(blake160(lockScript));

        let transfer: TransferAsset;
        beforeEach(async function() {
            const recipient = AssetAddress.fromTypeAndPayload(
                0,
                lockScriptHash,
                {
                    networkId: node.sdk.networkId
                }
            );

            const asset = await node.mintAsset({ supply: 1, recipient });

            transfer = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
        });

        it("unlock with the first key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, aliceSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, bobSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, carolSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the unknown key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, daveSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });
    });

    describe("2 of 3", async function() {
        const lockScript = Buffer.from([
            PUSH,
            2,
            PUSHB,
            64,
            ...Buffer.from(alicePublic, "hex"),
            PUSHB,
            64,
            ...Buffer.from(bobPublic, "hex"),
            PUSHB,
            64,
            ...Buffer.from(carolPublic, "hex"),
            PUSH,
            3,
            CHKMULTISIG
        ]);
        const lockScriptHash = new H160(blake160(lockScript));

        let transfer: TransferAsset;
        beforeEach(async function() {
            const recipient = AssetAddress.fromTypeAndPayload(
                0,
                lockScriptHash,
                {
                    networkId: node.sdk.networkId
                }
            );

            const asset = await node.mintAsset({ supply: 1, recipient });

            transfer = node.sdk.core
                .createTransferAssetTransaction()
                .addInputs(asset)
                .addOutputs({
                    quantity: 1,
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
        });

        it("unlock with the first and the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the first and the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, carolSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second and the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, bobSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, carolSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the second and the first key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, bobSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock with the third and the first key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, carolSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock with the third and the second key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, carolSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the first key is unknown", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, daveSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the second key is unknown", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, daveSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the same key signs twice", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: { input: "all", output: "all" },
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            const tag = encodeSignatureTag({ input: "all", output: "all" });
            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(
                    Buffer.from([
                        PUSHB,
                        tag.byteLength,
                        ...tag,
                        PUSHB,
                        65,
                        ...Buffer.from(signature1, "hex"),
                        PUSHB,
                        65,
                        ...Buffer.from(signature2, "hex")
                    ])
                );

            await expectTransactionFail(node, transfer);
        });
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
