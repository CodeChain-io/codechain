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

import * as chai from "chai";
import { expect } from "chai";
import * as chaiAsPromised from "chai-as-promised";
import * as fs from "fs";
import "mocha";
import * as path from "path";

import mkdirp = require("mkdirp");
import { validators } from "../../../tendermint.dynval/constants";
import { PromiseExpect } from "../../helper/promise";
import { setTermTestTimeout, withNodes } from "../setup";

chai.use(chaiAsPromised);

const SNAPSHOT_CONFIG = `${__dirname}/../../../tendermint.dynval/snapshot-config.yml`;
const SNAPSHOT_PATH = `${__dirname}/../../../../snapshot/`;

describe("Snapshot for Tendermint with Dynamic Validator", function() {
    const promiseExpect = new PromiseExpect();
    const snapshotValidators = validators.slice(0, 3);

    describe("Snapshot", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 3
            },
            validators: snapshotValidators.map((signer, index) => ({
                signer,
                delegation: 5000,
                deposit: 10_000_000 - index // tie-breaker
            })),
            modify: () => {
                mkdirp.sync(SNAPSHOT_PATH);
                const snapshotPath = fs.mkdtempSync(SNAPSHOT_PATH);
                return {
                    additionalArgv: [
                        "--snapshot-path",
                        snapshotPath,
                        "--config",
                        SNAPSHOT_CONFIG
                    ],
                    nodeAdditionalProperties: {
                        snapshotPath
                    }
                };
            }
        });

        it("should be exist after some time", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });
            const termMetadata = await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });

            const blockHash = (await nodes[0].sdk.rpc.chain.getBlockHash(
                termMetadata.lastTermFinishedBlockNumber
            ))!;
            const stateRoot = (await nodes[0].sdk.rpc.chain.getBlock(
                blockHash
            ))!.stateRoot;
            expect(
                fs.existsSync(
                    path.join(
                        nodes[0].snapshotPath,
                        blockHash.toString(),
                        stateRoot.toString()
                    )
                )
            ).to.be.true;
        });
    });
    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
