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
import { SignedParcel, U256 } from "codechain-sdk/lib/core/classes";

import CodeChain from "../helper/spawn";

const RLP = require("rlp");

const ERROR = {
    NOT_ENOUGH_BALANCE: {
        code: -32032,
        data: expect.anything(),
        message: expect.anything()
    },
    INVALID_RLP_TOO_BIG: {
        code: -32009,
        data: "RlpIsTooBig",
        message: expect.anything()
    },
    INVALID_RLP_TOO_SHORT: {
        code: -32009,
        data: "RlpIsTooShort",
        message: expect.anything()
    },
    INVALID_RLP_INVALID_LENGTH: {
        code: -32009,
        data: "RlpInvalidLength",
        message: expect.anything()
    },
    INVALID_RLP_UNEXPECTED_ACTION_PREFIX: {
        code: -32009,
        data: expect.stringContaining("Unexpected action prefix"),
        message: expect.anything()
    },
    INVALID_RLP_INCORRECT_LIST_LEN: {
        code: -32009,
        data: "RlpIncorrectListLen",
        message: expect.anything()
    },
    TOO_LOW_FEE: {
        code: -32033,
        data: expect.anything(),
        message: expect.anything()
    },
    INVALID_NETWORK_ID: {
        code: -32036,
        data: expect.anything(),
        message: expect.anything()
    }
};

describe("solo - 1 node", () => {
    const secret =
        "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
    const address = PlatformAddress.fromAccountId(
        SDK.util.getAccountIdFromPrivate(secret)
    );
    const recipient = "tccqruq09sfgax77nj4gukjcuq69uzeyv0jcs7vzngg";

    let node: CodeChain;
    beforeAll(async () => {
        node = new CodeChain();
        await node.start();
    });

    describe("Sending invalid parcels over the limits (general)", () => {
        let parcelEncoded: any[];
        beforeEach(async () => {
            const nonce = await node.sdk.rpc.chain.getNonce(address);
            const parcel = node.sdk.core
                .createPaymentParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret,
                    fee: 10,
                    nonce
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        test.each(["0x01" + "0".repeat(64), "0x" + "f".repeat(128)])(
            "nonce: %p",
            async (nonce, done) => {
                parcelEncoded[0] = nonce;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                    done();
                }
            }
        );

        test.each(["0x01" + "0".repeat(64), "0x" + "f".repeat(128)])(
            "fee: %p",
            async (fee, done) => {
                parcelEncoded[1] = fee;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                    done();
                }
            }
        );

        test.each(["tcc", "a", "ac"])(
            "networkId: %p",
            async (networkId, done) => {
                parcelEncoded[2] = networkId;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    if (networkId.length !== 2)
                        expect(e).toEqual(ERROR.INVALID_RLP_INVALID_LENGTH);
                    else expect(e).toEqual(ERROR.INVALID_NETWORK_ID);
                    done();
                }
            }
        );

        test.each([0, 7, 100])(
            "action (invalid type): %p",
            async (action, done) => {
                parcelEncoded[3] = [action];
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    expect(e).toEqual(
                        ERROR.INVALID_RLP_UNEXPECTED_ACTION_PREFIX
                    );
                    done();
                }
            }
        );

        test.each([
            [[1, 3]],
            [[1, 5]],
            [[2, 2]],
            [[2, 4]],
            [[3, 1]],
            [[3, 3]],
            [[4, 2]],
            [[5, 2]],
            [[5, 4]],
            [[6, 2]],
            [[6, 4]]
        ])("action (type / invalid length): %p", async (action, done) => {
            const { 0: action_type, 1: action_length } = action;
            parcelEncoded[3] = Array(action_length).fill(action_type);
            try {
                await node.sendSignedParcelWithRlpBytes(
                    RLP.encode(parcelEncoded)
                );
                done.fail();
            } catch (e) {
                expect(e).toEqual(ERROR.INVALID_RLP_INCORRECT_LIST_LEN);
                done();
            }
        });

        test.each([
            "0x00",
            "0x1" + "0".repeat(127),
            "0x1" + "0".repeat(130),
            "0x" + "f".repeat(131)
        ])("signature: %p", async (sig, done) => {
            parcelEncoded[4] = sig;
            try {
                await node.sendSignedParcelWithRlpBytes(
                    RLP.encode(parcelEncoded)
                );
                done.fail();
            } catch (e) {
                if (sig.length < 132)
                    expect(e).toEqual(ERROR.INVALID_RLP_TOO_SHORT);
                else expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                done();
            }
        });
    });

    test.skip("Sending invalid parcels over the limits (in action 1: AssetTransactionGroup)", done => done.fail("not implemented"));
    test.skip("Sending invalid parcels over the limits (in action 5: SetShardOwners)", done => done.fail("not implemented"));
    test.skip("Sending invalid parcels over the limits (in action 6: SetShardUsers)", done => done.fail("not implemented"));

    describe("Sending invalid parcels over the limits (in action 2: Payment)", () => {
        let parcelEncoded: any[];
        beforeEach(async () => {
            const nonce = await node.sdk.rpc.chain.getNonce(address);
            const parcel = node.sdk.core
                .createPaymentParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret,
                    fee: 10,
                    nonce
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        test.each(["0x1" + "0".repeat(40), "0x" + "f".repeat(38)])(
            "recipient: %p",
            async (recipient, done) => {
                parcelEncoded[3][1] = recipient;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    if (recipient.length < 42)
                        expect(e).toEqual(ERROR.INVALID_RLP_TOO_SHORT);
                    else expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                    done();
                }
            }
        );

        test.each(["0x01" + "0".repeat(64), "0x" + "f".repeat(128)])(
            "amount: %p",
            async (amount, done) => {
                parcelEncoded[3][2] = amount;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                    done();
                }
            }
        );
    });

    describe("Sending invalid parcels over the limits (in action 3: SetRegularKey)", () => {
        let parcelEncoded: any[];
        beforeEach(async () => {
            const privKey = node.sdk.util.generatePrivateKey();
            const key = node.sdk.util.getPublicFromPrivate(privKey);
            const nonce = await node.sdk.rpc.chain.getNonce(address);
            const parcel = node.sdk.core
                .createSetRegularKeyParcel({
                    key
                })
                .sign({
                    secret,
                    fee: 10,
                    nonce
                });
            parcelEncoded = parcel.toEncodeObject();
        });

        test.each(["0x01" + "0".repeat(128), "0x" + "f".repeat(126)])(
            "amount: %p",
            async (key, done) => {
                parcelEncoded[3][1] = key;
                try {
                    await node.sendSignedParcelWithRlpBytes(
                        RLP.encode(parcelEncoded)
                    );
                    done.fail();
                } catch (e) {
                    if (key.length < 130)
                        expect(e).toEqual(ERROR.INVALID_RLP_TOO_SHORT);
                    else expect(e).toEqual(ERROR.INVALID_RLP_TOO_BIG);
                    done();
                }
            }
        );
    });

    test.each([0, 9])(
        "Sending invalid parcels (low fee): %p",
        async (fee, done) => {
            const nonce = await node.sdk.rpc.chain.getNonce(address);
            const parcel = node.sdk.core
                .createPaymentParcel({
                    recipient,
                    amount: 0
                })
                .sign({
                    secret,
                    fee,
                    nonce
                });
            try {
                await node.sdk.rpc.chain.sendSignedParcel(parcel);
                done.fail();
            } catch (e) {
                expect(e).toEqual(ERROR.TOO_LOW_FEE);
                done();
            }
        }
    );

    afterAll(async () => {
        await node.clean();
    });
});
