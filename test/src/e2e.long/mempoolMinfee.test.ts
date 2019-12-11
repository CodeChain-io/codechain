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

import { expect } from "chai";
import "mocha";
import {
    validator0Address,
    validator1Address,
    validator2Address
} from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("MemPoolMinFees", async function() {
    const chain = `${__dirname}/../scheme/tendermint-int.json`;
    const config1 = `test/src/config/mem-pool-min-fee1.toml`;
    const config2 = `test/src/config/mem-pool-min-fee2.toml`;
    let valNode1WithMinPayFee150: CodeChain;
    let valNode2WithMinPayFee200: CodeChain;
    let valNode3: CodeChain;
    const nonValAddress = "tccq83wm6sjyklkd4utk6hjmewsaccgvzk5sck8cs2y";
    let nonValNode: CodeChain;

    beforeEach(async function() {
        valNode1WithMinPayFee150 = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                validator0Address.toString(),
                "--password-path",
                "test/tendermint/password.json",
                "--force-sealing",
                "--config",
                config1
            ],
            additionalKeysPath: "tendermint/keys"
        });

        valNode2WithMinPayFee200 = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                validator1Address.toString(),
                "--password-path",
                "test/tendermint/password.json",
                "--force-sealing",
                "--config",
                config2
            ],
            additionalKeysPath: "tendermint/keys"
        });

        valNode3 = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                validator2Address.toString(),
                "--password-path",
                "test/tendermint/password.json",
                "--force-sealing"
            ],
            additionalKeysPath: "tendermint/keys"
        });

        await valNode1WithMinPayFee150.start();
        await valNode2WithMinPayFee200.start();
        await valNode3.start();

        await valNode1WithMinPayFee150.connect(valNode2WithMinPayFee200);
        await valNode1WithMinPayFee150.connect(valNode3);
        await valNode2WithMinPayFee200.connect(valNode3);

        await valNode1WithMinPayFee150.waitPeers(2);
        await valNode2WithMinPayFee200.waitPeers(2);
        await valNode3.waitPeers(2);
    });

    afterEach(async function() {
        await valNode1WithMinPayFee150.clean();
        await valNode2WithMinPayFee200.clean();
        await valNode3.clean();
    });

    it("A node should accept a transaction with a fee higher than the node's mem pool minimum fee and the block should be propagated", async function() {
        const tx = await valNode1WithMinPayFee150.sendPayTx({
            seq: 0,
            fee: 175,
            quantity: 100_000,
            recipient: validator0Address
        });
        await valNode1WithMinPayFee150.waitBlockNumber(2);
        await valNode2WithMinPayFee200.waitBlockNumber(2);
        await valNode3.waitBlockNumber(2);

        expect(
            await valNode1WithMinPayFee150.sdk.rpc.chain.containsTransaction(
                tx.hash()
            )
        ).to.be.true;
        expect(
            await valNode2WithMinPayFee200.sdk.rpc.chain.containsTransaction(
                tx.hash()
            )
        ).to.be.true;
        expect(await valNode3.sdk.rpc.chain.containsTransaction(tx.hash())).to
            .be.true;
    });

    it("Connected validators should reject a transaction with a fee lower than the nodes' mem pool minimum fees", async function() {
        nonValNode = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                nonValAddress,
                "--password-path",
                `test/custom.minfee/${nonValAddress}/password.json`,
                "--force-sealing"
            ],
            additionalKeysPath: `tendermint.dynval/${nonValAddress}/keys`
        });
        await nonValNode.start();
        await nonValNode.connect(valNode1WithMinPayFee150);
        await nonValNode.connect(valNode2WithMinPayFee200);

        await nonValNode.waitPeers(2);
        await valNode1WithMinPayFee150.waitPeers(3);
        await valNode2WithMinPayFee200.waitPeers(3);

        const nodeArray = [
            valNode1WithMinPayFee150,
            valNode2WithMinPayFee200,
            valNode3,
            nonValNode
        ];

        const txShouldBeRejected = await nonValNode.sendPayTx({
            fee: 145,
            quantity: 100_000,
            recipient: validator0Address
        });

        const txShouldBeAccepted = await nonValNode.sendPayTx({
            fee: 210,
            quantity: 100_000,
            recipient: validator0Address
        });

        await Promise.all(nodeArray.map(node => node.waitBlockNumber(3)));
        const expectedTrues = await Promise.all(
            nodeArray.map(node =>
                node.sdk.rpc.chain.containsTransaction(
                    txShouldBeAccepted.hash()
                )
            )
        );
        const expectedFalses = await Promise.all(
            nodeArray.map(node =>
                node.sdk.rpc.chain.containsTransaction(
                    txShouldBeRejected.hash()
                )
            )
        );

        expectedTrues.map(val => expect(val).to.be.true);
        expectedFalses.map(val => expect(val).to.be.false);

        await nonValNode.clean();
    });
});
