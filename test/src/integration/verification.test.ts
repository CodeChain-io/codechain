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

import CodeChain from "../helper/spawn";
import { ERROR, errorMatcher } from "../helper/error";
import { faucetAddress, faucetSecret } from "../helper/constants";

import "mocha";
import { expect } from "chai";
import { AssetTransferAddress } from "codechain-primitives/lib";
import {
    AssetScheme,
    AssetTransferInput,
    AssetTransferOutput
} from "codechain-sdk/lib/core/classes";

const RLP = require("rlp");

describe("solo - 1 node", function() {
    const recipient = "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw";

    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("Sending invalid parcels over the limits (general)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createPayParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
            seq
        ) {
            it(`seq: ${seq}`, async function() {
                parcelEncoded[0] = seq;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
            fee
        ) {
            it(`fee: ${fee}`, async function() {
                parcelEncoded[1] = fee;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        ["tcc", "a", "ac"].forEach(function(networkId) {
            it(`networkId: ${networkId}`, async function() {
                parcelEncoded[2] = networkId;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    if (networkId.length !== 2)
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_INVALID_LENGTH)
                        );
                    else
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_NETWORK_ID)
                        );
                }
            });
        });

        [0, 10, 100].forEach(function(action) {
            it(`action (invalid type): ${action}`, async function() {
                parcelEncoded[3] = [action];
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_UNEXPECTED_ACTION_PREFIX)
                    );
                }
            });
        });

        [
            { actionType: 1, actionLength: 4 },
            { actionType: 2, actionLength: 2 },
            { actionType: 2, actionLength: 4 },
            { actionType: 3, actionLength: 1 },
            { actionType: 3, actionLength: 3 },
            { actionType: 4, actionLength: 2 },
            { actionType: 5, actionLength: 2 },
            { actionType: 5, actionLength: 4 },
            { actionType: 6, actionLength: 2 },
            { actionType: 6, actionLength: 4 }
        ].forEach(function(params: {
            actionType: number;
            actionLength: number;
        }) {
            const { actionType, actionLength } = params;
            it(`action (type / invalid length): ${actionType}, ${actionLength}`, async function() {
                parcelEncoded[3] = Array(actionLength).fill(actionType);
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_INCORRECT_LIST_LEN)
                    );
                }
            });
        });

        [
            "0x00",
            "0x1" + "0".repeat(127),
            "0x1" + "0".repeat(130),
            "0x" + "f".repeat(131)
        ].forEach(function(sig) {
            it(`signature: ${sig}`, async function() {
                parcelEncoded[4] = sig;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    if (sig.length < 132)
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_SHORT)
                        );
                    else
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                }
            });
        });
    });

    describe("Sending invalid parcels over the limits (in action 1: AssetTransaction)", function() {
        let scheme: AssetScheme;
        let input: AssetTransferInput;
        let output: AssetTransferOutput;
        let recipient: AssetTransferAddress;

        before(async function() {
            recipient = await node.createP2PKHAddress();
            scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "Valid metadata",
                amount: 10
            });
            input = node.sdk.core.createAssetTransferInput({
                assetOutPoint: {
                    transactionHash: "0x" + "0".repeat(64),
                    index: 0,
                    assetType: "0x" + "1".repeat(64),
                    amount: 12345
                },
                timelock: {
                    type: "block",
                    value: 0
                }
            });
            output = node.sdk.core.createAssetTransferOutput({
                assetType: "0x" + "0".repeat(64),
                amount: 12345,
                recipient
            });
        });

        describe("In assetMintTransction", function() {
            let parcelEncoded: any[];
            beforeEach(async function() {
                const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
                const tx = node.sdk.core.createAssetMintTransaction({
                    scheme,
                    recipient
                });
                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: tx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq
                    });
                parcelEncoded = parcel.toEncodeObject();
            });

            [65536, 100000].forEach(function(shardId) {
                it(`shardId: ${shardId}`, async function() {
                    parcelEncoded[3][1][2] = shardId;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
                amount
            ) {
                it(`amount: ${amount}`, async function() {
                    parcelEncoded[3][1][6][0] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(40), "0x" + "f".repeat(41)].forEach(function(
                lockScriptHash
            ) {
                it(`lockScriptHash: ${lockScriptHash}`, async function() {
                    parcelEncoded[3][1][4] = lockScriptHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            it("parameters");
            it("registrar");
        });

        describe("In assetTransferTransaction", function() {
            let parcelEncoded: any[];
            beforeEach(async function() {
                const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
                const tx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addBurns(input)
                    .addInputs(input)
                    .addOutputs(output);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: tx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq
                    });
                parcelEncoded = parcel.toEncodeObject();
            });

            ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
                amount
            ) {
                it(`amount: ${amount}`, async function() {
                    // Burn
                    parcelEncoded[3][1][2][0][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                    // Input
                    parcelEncoded[3][1][3][0][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }

                    // Output
                    parcelEncoded[3][1][4][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                transactionHash
            ) {
                it(`transactionHash: ${transactionHash}`, async function() {
                    // Burn
                    parcelEncoded[3][1][2][0][0][0] = transactionHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                    // Input
                    parcelEncoded[3][1][3][0][0][0] = transactionHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                assetType
            ) {
                it(`assetType: ${assetType}`, async function() {
                    // Burn
                    parcelEncoded[3][1][2][0][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                    // Input
                    parcelEncoded[3][1][3][0][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }

                    // Output
                    parcelEncoded[3][1][4][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(40), "0x" + "f".repeat(41)].forEach(function(
                lockScriptHash
            ) {
                it(`lockScriptHash: ${lockScriptHash}`, async function() {
                    parcelEncoded[3][1][4][0][0] = lockScriptHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            it("parameters");
            it("index");
            it("timelock");
            it("lockscript/unlockscript");
        });

        describe("In assetComposeTransaction", function() {
            let parcelEncoded: any[];
            beforeEach(async function() {
                const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
                const tx = node.sdk.core.createAssetComposeTransaction({
                    scheme,
                    inputs: [input],
                    recipient
                });
                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: tx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq
                    });
                parcelEncoded = parcel.toEncodeObject();
            });

            [65536, 100000].forEach(function(shardId) {
                it(`shardId: ${shardId}`, async function() {
                    parcelEncoded[3][1][2] = shardId;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
                amount
            ) {
                it(`amount: ${amount}`, async function() {
                    // Input
                    parcelEncoded[3][1][6][0][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }

                    // Output
                    parcelEncoded[3][1][9][0] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                assetType
            ) {
                it(`assetType: ${assetType}`, async function() {
                    // Input
                    parcelEncoded[3][1][6][0][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(40), "0x" + "f".repeat(41)].forEach(function(
                lockScriptHash
            ) {
                it(`lockScriptHash: ${lockScriptHash}`, async function() {
                    parcelEncoded[3][1][7] = lockScriptHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            it("parameters");
            it("index");
            it("timelock");
            it("registrar");
            it("lockscript/unlockscript");
        });

        describe("In assetDecomposeTransaction", function() {
            let parcelEncoded: any[];
            beforeEach(async function() {
                const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
                const tx = node.sdk.core.createAssetDecomposeTransaction({
                    input,
                    outputs: [output]
                });

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: tx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq
                    });
                parcelEncoded = parcel.toEncodeObject();
            });

            ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
                amount
            ) {
                it(`amount: ${amount}`, async function() {
                    // Input
                    parcelEncoded[3][1][2][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }

                    // Output
                    parcelEncoded[3][1][3][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                transactionHash
            ) {
                it(`transactionHash: ${transactionHash}`, async function() {
                    // Input
                    parcelEncoded[3][1][2][0][0] = transactionHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                assetType
            ) {
                it(`assetType: ${assetType}`, async function() {
                    // Input
                    parcelEncoded[3][1][2][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }

                    // Output
                    parcelEncoded[3][1][3][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(40), "0x" + "f".repeat(41)].forEach(function(
                lockScriptHash
            ) {
                it(`lockScriptHash: ${lockScriptHash}`, async function() {
                    parcelEncoded[3][1][3][0][0] = lockScriptHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            it("parameters");
            it("index");
            it("timelock");
            it("lockscript/unlockscript");
        });

        describe("In assetUnwrapCCCTransaction", function() {
            let parcelEncoded: any[];
            beforeEach(async function() {
                const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
                const tx = node.sdk.core.createAssetUnwrapCCCTransaction({
                    burn: input
                });

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: tx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq
                    });
                parcelEncoded = parcel.toEncodeObject();
            });

            ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
                amount
            ) {
                it(`amount: ${amount}`, async function() {
                    parcelEncoded[3][1][2][0][3] = amount;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                transactionHash
            ) {
                it(`transactionHash: ${transactionHash}`, async function() {
                    parcelEncoded[3][1][2][0][0] = transactionHash;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            ["0x1" + "0".repeat(64), "0x" + "f".repeat(65)].forEach(function(
                assetType
            ) {
                it(`assetType: ${assetType}`, async function() {
                    parcelEncoded[3][1][2][0][2] = assetType;
                    try {
                        await node.sendSignedParcelWithRlpBytes(
                            RLP.encode(parcelEncoded)
                        );
                        expect.fail();
                    } catch (e) {
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                    }
                });
            });

            it("parameters");
            it("index");
            it("timelock");
            it("lockscript/unlockscript");
        });
    });

    describe("Sending invalid parcels over the limits (in action 2: Pay)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createPayParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        ["0x1" + "0".repeat(40), "0x" + "f".repeat(38)].forEach(function(
            recipient
        ) {
            it(`recipient: ${recipient}`, async function() {
                parcelEncoded[3][1] = recipient;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    if (recipient.length < 42)
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_SHORT)
                        );
                    else
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                }
            });
        });

        ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
            amount
        ) {
            it(`amount: ${amount}`, async function() {
                parcelEncoded[3][2] = amount;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });
    });

    describe("Sending invalid parcels over the limits (in action 3: SetRegularKey)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const privKey = node.sdk.util.generatePrivateKey();
            const key = node.sdk.util.getPublicFromPrivate(privKey);
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createSetRegularKeyParcel({
                    key
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        ["0x01" + "0".repeat(128), "0x" + "f".repeat(126)].forEach(function(
            key
        ) {
            it(`key: ${key}`, async function() {
                parcelEncoded[3][1] = key;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    if (key.length < 130)
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_SHORT)
                        );
                    else
                        expect(e).to.satisfy(
                            errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                        );
                }
            });
        });
    });

    describe("Sending invalid parcels over the limits (in action 5: SetShardOwners)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const account = await node.createPlatformAddress();
            const parcel = node.sdk.core
                .createSetShardOwnersParcel({
                    shardId: 0,
                    owners: [account]
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        [65536, 100000].forEach(function(shardId) {
            it(`shardId: ${shardId}`, async function() {
                parcelEncoded[3][1] = shardId;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        it("Owners");
    });

    describe("Sending invalid parcels over the limits (in action 6: SetShardUsers)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const account = await node.createPlatformAddress();
            const parcel = node.sdk.core
                .createSetShardUsersParcel({
                    shardId: 0,
                    users: [account]
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        [65536, 100000].forEach(function(shardId) {
            it(`shardId: ${shardId}`, async function() {
                parcelEncoded[3][1] = shardId;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        it("Users");
    });

    describe("Sending invalid parcels over the limits (in action 7: WrapCCC)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const account = await node.createPlatformAddress();
            const recipient = await node.createP2PKHAddress();
            const parcel = node.sdk.core
                .createWrapCCCParcel({
                    shardId: 0,
                    recipient,
                    amount: 10
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        [65536, 100000].forEach(function(shardId) {
            it(`shardId: ${shardId}`, async function() {
                parcelEncoded[3][1] = shardId;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        ["0x01" + "0".repeat(64), "0x" + "f".repeat(128)].forEach(function(
            amount
        ) {
            it(`amount: ${amount}`, async function() {
                parcelEncoded[3][4] = amount;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        ["0x1" + "0".repeat(40), "0x" + "f".repeat(41)].forEach(function(
            lockScriptHash
        ) {
            it(`lockScriptHash: ${lockScriptHash}`, async function() {
                parcelEncoded[3][2] = lockScriptHash;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_RLP_TOO_BIG)
                    );
                }
            });
        });

        it("parameters");
    });

    [0, 9].forEach(function(fee) {
        it(`Sending invalid parcels (low fee): ${fee}`, async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createPayParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret: faucetSecret,
                    fee,
                    seq
                });
            try {
                await node.sdk.rpc.chain.sendSignedParcel(parcel);
                expect.fail();
            } catch (e) {
                expect(e).to.satisfy(errorMatcher(ERROR.TOO_LOW_FEE));
            }
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
