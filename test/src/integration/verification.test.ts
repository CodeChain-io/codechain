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
                .createPaymentParcel({
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

        [0, 8, 100].forEach(function(action) {
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
            { actionType: 1, actionLength: 3 },
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

    it(
        "Sending invalid parcels over the limits (in action 1: AssetTransaction)"
    );
    it("Sending invalid parcels over the limits (in action 5: SetShardOwners)");
    it("Sending invalid parcels over the limits (in action 6: SetShardUsers)");
    it("Sending invalid parcels over the limits (in action 7: WrapCCC)");

    describe("Sending invalid parcels over the limits (in action 2: Payment)", function() {
        let parcelEncoded: any[];
        beforeEach(async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createPaymentParcel({
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

    [0, 9].forEach(function(fee) {
        it(`Sending invalid parcels (low fee): ${fee}`, async function() {
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const parcel = node.sdk.core
                .createPaymentParcel({
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
