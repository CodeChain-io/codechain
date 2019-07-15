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
import { PlatformAddress } from "codechain-sdk/lib/core/classes";
import { blake256, signEcdsa } from "codechain-sdk/lib/utils";
import "mocha";
import { bobAddress, faucetAddress, faucetSecret } from "../helper/constants";
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
        expect(await node.sdk.rpc.chain.containsTransaction(storeHash)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(storeHash)).not.null;

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

        const blockNumber = await node.getBestBlockNumber();
        const removeHash = await node.sdk.rpc.chain.sendSignedTransaction(
            remove
        );
        await node.waitBlockNumber(blockNumber + 1);
        expect(await node.sdk.rpc.chain.containsTransaction(removeHash)).be
            .true;
        expect(await node.sdk.rpc.chain.getTransaction(removeHash)).not.null;
    });

    it("storing with wrong certifier fails", async function() {
        const wrongPrivKey = node.sdk.util.generatePrivateKey();
        const signature = signEcdsa(
            blake256(RLP.encode(content)),
            wrongPrivKey
        );

        const blockNumber = await node.getBestBlockNumber();
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const pay = node.sdk.core
            .createPayTransaction({ recipient: bobAddress, quantity: 1 })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
        const store = node.sdk.core
            .createStoreTransaction({
                content,
                certifier: address,
                signature
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 1
            });
        await node.sdk.rpc.devel.stopSealing();
        await node.sdk.rpc.chain.sendSignedTransaction(pay);
        const storeHash = await node.sdk.rpc.chain.sendSignedTransaction(store);
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumber + 1);
        expect(await node.sdk.rpc.chain.getErrorHint(storeHash)).not.be.null;
    });

    it("storing with invalid signature fails", async function() {
        const blockNumber = await node.getBestBlockNumber();
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const pay = node.sdk.core
            .createPayTransaction({ recipient: bobAddress, quantity: 1 })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
        const store = node.sdk.core
            .createStoreTransaction({
                content,
                certifier: address,
                signature: "a".repeat(130)
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 1
            });

        await node.sdk.rpc.devel.stopSealing();
        await node.sdk.rpc.chain.sendSignedTransaction(pay);
        const storeHash = await node.sdk.rpc.chain.sendSignedTransaction(store);
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumber + 1);
        expect(await node.sdk.rpc.chain.getErrorHint(storeHash)).not.be.null;
    });

    it("removal on nothing fails", async function() {
        const blockNumber = await node.getBestBlockNumber();
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const pay = node.sdk.core
            .createPayTransaction({ recipient: bobAddress, quantity: 1 })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq
            });
        const remove = node.sdk.core
            .createRemoveTransaction({
                hash: makeRandomH256(),
                secret: privKey
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: seq + 1
            });

        await node.sdk.rpc.devel.stopSealing();
        await node.sdk.rpc.chain.sendSignedTransaction(pay);
        const removeHash = await node.sdk.rpc.chain.sendSignedTransaction(
            remove
        );
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumber + 1);
        expect(await node.sdk.rpc.chain.getErrorHint(removeHash)).not.be.null;
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
    });

    after(async function() {
        await node.clean();
    });
});
