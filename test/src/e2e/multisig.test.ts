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
    Asset,
    AssetAddress,
    H160,
    Script,
    TransferAsset,
    Transaction
} from "codechain-sdk/lib/core/classes";
import {
    blake160,
    encodeSignatureTag,
    signEcdsa,
    SignatureTag
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

function createUnlockScript(
    tag: SignatureTag,
    ...signatures: Array<string>
): Buffer {
    const encodedTag = encodeSignatureTag(tag);
    const inputArray = [PUSHB, encodedTag.byteLength, ...encodedTag];

    signatures.forEach((sigInstance: string) => {
        inputArray.push(PUSHB, 65, ...Buffer.from(sigInstance, "hex"));
    });

    return Buffer.from(inputArray);
}

function createLockScript(
    atLeast: number,
    total: number,
    ...publics: Array<string>
): Buffer {
    const inputArray = [PUSH, atLeast];
    publics.forEach((publicInstance: string) => {
        inputArray.push(PUSHB, 64, ...Buffer.from(publicInstance, "hex"));
    });
    inputArray.push(PUSH, total, CHKMULTISIG);

    return Buffer.from(inputArray);
}

describe("Multisig", function() {
    let node: CodeChain;
    const defaultTag: SignatureTag = { input: "all", output: "all" };

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("1 of 2", async function() {
        const lockScript = createLockScript(1, 2, alicePublic, bobPublic);
        const lockScriptHash = new H160(blake160(lockScript));
        let recipient: AssetAddress;
        let transfer: TransferAsset;
        beforeEach(async function() {
            recipient = AssetAddress.fromTypeAndPayload(0, lockScriptHash, {
                networkId: node.sdk.networkId
            });

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
            transfer.input(0)!.setLockScript(lockScript);
        });

        it("unlock with the first key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, aliceSecret);

            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));
            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, bobSecret);
            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the unknown key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, carolSecret);

            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            await expectTransactionFail(node, transfer);
        });

        describe("Test partial signing", async function() {
            let additionalAsset: Asset;
            beforeEach(async function() {
                additionalAsset = await node.mintAsset({
                    supply: 10,
                    recipient
                });
            });
            it("unlock with fixed inputs", async function() {
                const tag: SignatureTag = { input: "all", output: [0] };
                transfer.addInputs(additionalAsset);
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );

                transfer.addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
                transfer
                    .input(0)!
                    .setUnlockScript(createUnlockScript(tag, signature0));

                transfer
                    .input(1)!
                    .setUnlockScript(createUnlockScript(tag, signature1));

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });

            it("unlock with fixed outputs", async function() {
                const tag: SignatureTag = { input: "single", output: [0, 1] };
                transfer.addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHBurnAddress()
                });

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                transfer.addInputs(additionalAsset);
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                transfer
                    .input(0)!
                    .setUnlockScript(createUnlockScript(tag, signature0));

                transfer
                    .input(1)!
                    .setUnlockScript(createUnlockScript(tag, signature1));

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });

            it("unlock with both outputs and inputs dynamic", async function() {
                const tag0: SignatureTag = { input: "single", output: [0] };
                const tag1: SignatureTag = { input: "single", output: [1] };

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag: tag0,
                    type: "input",
                    index: 0
                });
                const signature0 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );

                transfer.addInputs(additionalAsset).addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHBurnAddress()
                });
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag: tag1,
                    type: "input",
                    index: 1
                });
                const signature1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );

                transfer
                    .input(0)!
                    .setUnlockScript(createUnlockScript(tag0, signature0));

                transfer
                    .input(1)!
                    .setUnlockScript(createUnlockScript(tag1, signature1));

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });
        });
    });

    describe("1 of 3", async function() {
        const lockScript = createLockScript(
            1,
            3,
            alicePublic,
            bobPublic,
            carolPublic
        );
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
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, aliceSecret);

            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, bobSecret);

            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, carolSecret);

            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the unknown key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature = signEcdsa(hashWithoutScript.value, daveSecret);

            transfer.input(0)!.setLockScript(lockScript);
            transfer
                .input(0)!
                .setUnlockScript(createUnlockScript(defaultTag, signature));

            await expectTransactionFail(node, transfer);
        });
    });

    describe("2 of 3", async function() {
        const lockScript = createLockScript(
            2,
            3,
            alicePublic,
            bobPublic,
            carolPublic
        );
        const lockScriptHash = new H160(blake160(lockScript));
        let recipient: AssetAddress;
        let transfer: TransferAsset;
        beforeEach(async function() {
            recipient = AssetAddress.fromTypeAndPayload(0, lockScriptHash, {
                networkId: node.sdk.networkId
            });

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
            transfer.input(0)!.setLockScript(lockScript);
        });

        it("unlock with the first and the second key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the first and the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, carolSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("unlock with the second and the third key", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, bobSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, carolSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            const hash = await node.sendAssetTransaction(transfer);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        });

        it("fail to unlock with the second and the first key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, bobSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock with the third and the first key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, carolSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock with the third and the second key - signature unordered", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, carolSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the first key is unknown", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, daveSecret);
            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the second key is unknown", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, daveSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, bobSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        it("fail to unlock if the same key signs twice", async function() {
            const hashWithoutScript = transfer.hashWithoutScript({
                tag: defaultTag,
                type: "input",
                index: 0
            });
            const signature1 = signEcdsa(hashWithoutScript.value, aliceSecret);
            const signature2 = signEcdsa(hashWithoutScript.value, aliceSecret);

            transfer
                .input(0)!
                .setUnlockScript(
                    createUnlockScript(defaultTag, signature1, signature2)
                );

            await expectTransactionFail(node, transfer);
        });

        describe("Test partial signing", async function() {
            let additionalAsset: Asset;
            beforeEach(async function() {
                additionalAsset = await node.mintAsset({
                    supply: 10,
                    recipient
                });
            });
            it("fail to add inputs after sign with all inputs tag", async function() {
                const tag: SignatureTag = { input: "all", output: [0] };
                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0_1 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const signature0_2 = signEcdsa(
                    hashWithoutScript0.value,
                    bobSecret
                );

                transfer.addInputs(additionalAsset).addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
                transfer.input(1)!.setLockScript(lockScript);
                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1_1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                const signature1_2 = signEcdsa(
                    hashWithoutScript1.value,
                    carolSecret
                );

                transfer
                    .input(0)!
                    .setUnlockScript(
                        createUnlockScript(
                            defaultTag,
                            signature0_1,
                            signature0_2
                        )
                    );

                transfer
                    .input(1)!
                    .setUnlockScript(
                        createUnlockScript(
                            defaultTag,
                            signature1_1,
                            signature1_2
                        )
                    );

                await expectTransactionFail(node, transfer);
            });

            it("unlock with fixed inputs", async function() {
                const tag: SignatureTag = { input: "all", output: [0] };
                transfer.addInputs(additionalAsset);
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0_1 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const signatrue0_2 = signEcdsa(
                    hashWithoutScript0.value,
                    bobSecret
                );

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1_1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                const signature1_2 = signEcdsa(
                    hashWithoutScript1.value,
                    carolSecret
                );

                transfer.addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHAddress()
                });
                transfer
                    .input(0)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature0_1, signatrue0_2)
                    );

                transfer
                    .input(1)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature1_1, signature1_2)
                    );

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });

            it("fail to add inputs after sign with all outputs tag", async function() {
                const tag: SignatureTag = { input: "single", output: "all" };
                transfer.addInputs(additionalAsset);
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0_1 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const signature0_2 = signEcdsa(
                    hashWithoutScript0.value,
                    bobSecret
                );

                transfer.addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHBurnAddress()
                });

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1_1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                const signature1_2 = signEcdsa(
                    hashWithoutScript1.value,
                    carolSecret
                );

                transfer
                    .input(0)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature0_1, signature0_2)
                    );

                transfer
                    .input(1)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature1_1, signature1_2)
                    );

                await expectTransactionFail(node, transfer);
            });

            it("unlock with fixed outputs", async function() {
                const tag: SignatureTag = { input: "single", output: "all" };
                transfer.addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHBurnAddress()
                });

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 0
                });
                const signature0_1 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const signature0_2 = signEcdsa(
                    hashWithoutScript0.value,
                    bobSecret
                );

                transfer.addInputs(additionalAsset);
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag,
                    type: "input",
                    index: 1
                });
                const signature1_1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                const signature1_2 = signEcdsa(
                    hashWithoutScript1.value,
                    carolSecret
                );

                transfer
                    .input(0)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature0_1, signature0_2)
                    );

                transfer
                    .input(1)!
                    .setUnlockScript(
                        createUnlockScript(tag, signature1_1, signature1_2)
                    );

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });

            it("unlock with both outputs and inputs dynamic", async function() {
                const tag0: SignatureTag = { input: "single", output: [0] };
                const tag1: SignatureTag = { input: "single", output: [1] };

                const hashWithoutScript0 = transfer.hashWithoutScript({
                    tag: tag0,
                    type: "input",
                    index: 0
                });
                const signature0_1 = signEcdsa(
                    hashWithoutScript0.value,
                    aliceSecret
                );
                const signatrue0_2 = signEcdsa(
                    hashWithoutScript0.value,
                    bobSecret
                );

                transfer.addInputs(additionalAsset).addOutputs({
                    quantity: 10,
                    assetType: additionalAsset.assetType,
                    shardId: additionalAsset.shardId,
                    recipient: await node.createP2PKHBurnAddress()
                });
                transfer.input(1)!.setLockScript(lockScript);

                const hashWithoutScript1 = transfer.hashWithoutScript({
                    tag: tag1,
                    type: "input",
                    index: 1
                });
                const signature1_1 = signEcdsa(
                    hashWithoutScript1.value,
                    bobSecret
                );
                const signature1_2 = signEcdsa(
                    hashWithoutScript1.value,
                    carolSecret
                );

                transfer
                    .input(0)!
                    .setUnlockScript(
                        createUnlockScript(tag0, signature0_1, signatrue0_2)
                    );

                transfer
                    .input(1)!
                    .setUnlockScript(
                        createUnlockScript(tag1, signature1_1, signature1_2)
                    );

                const hash = await node.sendAssetTransaction(transfer);
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
                expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            });
        });
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});
