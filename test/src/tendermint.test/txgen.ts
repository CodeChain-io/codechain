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

import {
    faucetAddress,
    faucetSecret,
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address,
    validator0Secret,
    validator1Secret,
    validator2Secret,
    validator3Secret
} from "../helper/constants";
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";
const {
    Worker,
    isMainThread,
    parentPort,
    workerData
} = require("worker_threads");
const path = require("path");

const RLP = require("rlp");

(async () => {
    let nodes: CodeChain[];

    const validatorAddresses = [
        validator0Address,
        validator1Address,
        validator2Address,
        validator3Address
    ];
    let promises = [];
    const validatorSecrets = [
        validator0Secret,
        validator1Secret,
        validator2Secret,
        validator3Secret
    ];

    for (let index = 0; index < 4; index += 1) {
        const worker = new Worker(
            path.resolve(__dirname, "./txgen_worker.js"),
            {
                workerData: {
                    wname: `${index}`,
                    secret: validatorSecrets[index],
                    seqStart: 50000,
                    seqEnd: 60000,
                    filePrefix: `${index}`
                }
            }
        );

        let workerPromise = new Promise((resolve, reject) => {
            worker.on("error", reject);
            worker.on("exit", (code: any) => {
                if (code !== 0) {
                    reject(new Error(`Worker stopped with exit code ${code}`));
                }
            });
        });
        promises.push(workerPromise);
    }
    await Promise.all(promises);
})().catch(console.error);

