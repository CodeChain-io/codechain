// Copyright 2018-2019 Kodebox, Inc.
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

import { expect } from "chai";
import { MintAsset } from "codechain-sdk/lib/core/classes";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("ChangeAssetScheme", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    let mint: MintAsset;
    beforeEach(async function() {
        const recipient = await node.createP2PKHAddress();
        const scheme = node.sdk.core.createAssetScheme({
            registrar: faucetAddress,
            shardId: 0,
            metadata: "",
            supply: 10
        });
        mint = node.sdk.core.createMintAssetTransaction({
            scheme,
            recipient
        });
        const hash = await node.sendAssetTransaction(mint);
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
    });

    it("successful", async function() {
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        const changeAssetScheme = node.sdk.core.createChangeAssetSchemeTransaction(
            {
                shardId: 0,
                assetType: mint.getAssetType(),
                scheme: {
                    metadata: "A",
                    allowedScriptHashes: []
                },
                approvals: []
            }
        );
        const signedChangeAssetScheme = changeAssetScheme.sign({
            secret: faucetSecret,
            seq,
            fee: 10
        });
        const hash = await node.sdk.rpc.chain.sendSignedTransaction(
            signedChangeAssetScheme
        );
        expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
    });

    it("Changing to another scheme and set back to the original", async function() {
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        const changeAssetScheme0 = node.sdk.core.createChangeAssetSchemeTransaction(
            {
                shardId: 0,
                assetType: mint.getAssetType(),
                scheme: {
                    metadata: "A",
                    registrar: faucetAddress,
                    allowedScriptHashes: []
                },
                approvals: []
            }
        );
        const signedChangeAssetScheme0 = changeAssetScheme0.sign({
            secret: faucetSecret,
            seq,
            fee: 10
        });
        const hash0 = await node.sdk.rpc.chain.sendSignedTransaction(
            signedChangeAssetScheme0
        );
        expect(await node.sdk.rpc.chain.getTransaction(hash0)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash0)).be.true;

        const changeAssetScheme1 = node.sdk.core.createChangeAssetSchemeTransaction(
            {
                shardId: 0,
                assetType: mint.getAssetType(),
                scheme: {
                    metadata: "B",
                    registrar: faucetAddress,
                    allowedScriptHashes: []
                },
                seq: 1,
                approvals: []
            }
        );
        const signedChangeAssetScheme1 = changeAssetScheme1.sign({
            secret: faucetSecret,
            seq: seq + 1,
            fee: 10
        });
        const hash1 = await node.sdk.rpc.chain.sendSignedTransaction(
            signedChangeAssetScheme1
        );
        expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;

        const changeAssetScheme2 = node.sdk.core.createChangeAssetSchemeTransaction(
            {
                shardId: 0,
                assetType: mint.getAssetType(),
                seq: 2,
                scheme: {
                    metadata: "A",
                    registrar: faucetAddress,
                    allowedScriptHashes: []
                },
                approvals: []
            }
        );
        const signedChangeAssetScheme2 = changeAssetScheme2.sign({
            secret: faucetSecret,
            seq: seq + 2,
            fee: 10
        });
        const hash2 = await node.sdk.rpc.chain.sendSignedTransaction(
            signedChangeAssetScheme2
        );
        expect(await node.sdk.rpc.chain.getTransaction(hash2)).not.null;
        expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;
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
