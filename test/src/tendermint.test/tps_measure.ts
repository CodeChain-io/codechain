// Copyright 2020 Kodebox, Inc.
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
import { wait } from "../helper/promise";

interface BlockInfo {
    timestamp: number;
    transactionCount: number;
}

class TPSSection {
    private sectionLength: number;
    private records: Array<BlockInfo>;
    constructor(size: number) {
        this.sectionLength = size;
        this.records = new Array();
    }

    public accumulate = (info: BlockInfo) => {
        console.log(`timestamp: ${info.timestamp} and contains ${info.transactionCount} transactions`);
        if (this.records.push(info) > this.sectionLength + 1) {
            this.records.shift();
        }
    };

    public tps = (): number => {
        if (this.records.length < 2) {
            return 0;
        } else {
            const transactionExecuted =
                this.records.reduce(
                    (accum, current) => accum + current.transactionCount,
                    0
                ) - this.records[this.records.length - 1].transactionCount;
            const timeElapsed =
                this.records[this.records.length - 1].timestamp -
                this.records[0].timestamp;
            return transactionExecuted / timeElapsed;
        }
    };

    public averageTargetLength = (): number => {
        return this.records.length - 1;
    };
}

(async () => {
    const sdk = new SDK({
        server: process.env.SERVER || "http://192.168.1.101:2487",
        networkId: process.env.NETWORK_ID || "bc"
    });
    const sectionLength = parseInt(process.env.SECTION_LEN || "10", 10);
    const tpsSection = new TPSSection(sectionLength);
    const getBlockInfo = async (blockNumber: number): Promise<BlockInfo> => {
        const bestBlock = (await sdk.rpc.chain.getBlock(blockNumber))!;
        return {
            timestamp: bestBlock.timestamp,
            transactionCount: bestBlock.transactions.length
        };
    };

    let bestBlockNumber = await sdk.rpc.chain.getBestBlockNumber();
    tpsSection.accumulate(await getBlockInfo(bestBlockNumber));
    await wait(500);

    while (true) {
        if ((await sdk.rpc.chain.getBestBlockNumber()) === bestBlockNumber) {
            await wait(500);
        } else {
            bestBlockNumber = bestBlockNumber + 1;
            console.log(`current best block number is ${bestBlockNumber}`);
            tpsSection.accumulate(await getBlockInfo(bestBlockNumber));
            console.log(
                `current average tps for consecutive ${tpsSection.averageTargetLength()} is ${tpsSection.tps()}`
            );
        }
    }
})().catch(console.error);
