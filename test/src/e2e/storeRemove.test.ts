// Copyright 2018-2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// pubKeylished by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

import { expect } from "chai";
import "../helper/chai-similar";
import { PlatformAddress } from "codechain-sdk/lib/core/classes";
import { blake256, signEcdsa } from "codechain-sdk/lib/utils";
import * as _ from "lodash";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("store & remove", function() {
    let node: CodeChain;
    let privKey: string;
    let address: PlatformAddress;

    const content = "CodeChain";

    before(async function() {
        node = new CodeChain();
        await node.start();

        privKey = node.sdk.util.generatePrivateKey();
        const pubKey = node.sdk.util.getPublicFromPrivate(privKey);
        address = PlatformAddress.fromPublic(pubKey, { networkId: "tc" });
    });

    it("successfully", async function() {
        const store = node.sdk.core
            .createStoreTransaction({
                content,
                secret: privKey
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });

        const storeHash = await node.sdk.rpc.chain.sendSignedTransaction(store);
        const invoice1 = await node.sdk.rpc.chain.getInvoice(storeHash, {
            timeout: 300 * 1000
        });
        expect(invoice1).not.to.be.null;
        expect(invoice1!.success).to.be.true;

        const text = await node.sdk.rpc.chain.getText(storeHash);
        expect(text).not.to.be.null;
        expect(text!.content).to.equal(content);
        expect(text!.certifier).to.deep.equal(address);

        const remove = node.sdk.core
            .createRemoveTransaction({
                hash: storeHash,
                secret: privKey
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });

        const removeHash = await node.sdk.rpc.chain.sendSignedTransaction(
            remove
        );
        const invoice2 = await node.sdk.rpc.chain.getInvoice(removeHash, {
            timeout: 300 * 1000
        });
        expect(invoice2).not.to.be.null;
        expect(invoice2!.success).to.be.true;
    });

    it("storing with wrong certifier fails", async function() {
        const wrongPrivKey = node.sdk.util.generatePrivateKey();
        const { r, s, v } = signEcdsa(
            blake256(RLP.encode(content)),
            wrongPrivKey
        );
        const signature = `${_.padStart(r, 64, "0")}${_.padStart(
            s,
            64,
            "0"
        )}${_.padStart(v.toString(16), 2, "0")}`;

        const store = node.sdk.core
            .createStoreTransaction({
                content,
                certifier: address,
                signature
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });
        const storeHash = await node.sdk.rpc.chain.sendSignedTransaction(store);
        const invoice = await node.sdk.rpc.chain.getInvoice(storeHash, {
            timeout: 1000
        });
        expect(invoice).to.be.similarTo({
            success: false,
            error: {
                type: "TextVerificationFail",
                content: "Certifier and signer are different"
            }
        });
    });

    it("storing with invalid signature fails", async function() {
        const store = node.sdk.core
            .createStoreTransaction({
                content,
                certifier: address,
                signature: "a".repeat(130)
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });

        const storeHash = await node.sdk.rpc.chain.sendSignedTransaction(store);
        const invoice = await node.sdk.rpc.chain.getInvoice(storeHash, {
            timeout: 1000
        });
        expect(invoice).to.be.similarTo({
            success: false,
            error: {
                type: "TextVerificationFail",
                content: "Invalid Signature"
            }
        });
    });

    it("removal on nothing fails", async function() {
        const remove = node.sdk.core
            .createRemoveTransaction({
                hash: makeRandomH256(),
                secret: privKey
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });

        const removeHash = await node.sdk.rpc.chain.sendSignedTransaction(
            remove
        );
        const invoice = await node.sdk.rpc.chain.getInvoice(removeHash, {
            timeout: 300 * 1000
        });
        expect(invoice).not.to.be.null;
        expect(invoice!.success).to.be.false;
        expect(invoice!.error!.type).to.equal("TextNotExist");
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
    });

    after(async function() {
        await node.clean();
    });
});
