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

import { faucetSecret, faucetAddress } from "../helper/constants";
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";
import {SDK} from "codechain-sdk";

(async () => {
    const node = new CodeChain({
        chain: "solo",
    });
    await node.start();
    const sdk = node.sdk;
    const numTransactions = 10000;

    const baseSeq = await sdk.rpc.chain.getSeq(faucetAddress);
    const transactions = [];

    for (let i = 0; i < numTransactions; i++) {
        if (i % 1000 === 0) {
            console.log(`${i}`);
        }
        const value = makeRandomH256();
        const accountId = sdk.util.getAccountIdFromPrivate(value);
        const recipient = sdk.core.classes.PlatformAddress.fromAccountId(
            accountId,
            { networkId: "tc" }
        );
        const transaction = sdk.core
            .createPayTransaction({
                recipient,
                quantity: 1
            })
            .sign({
                secret: faucetSecret,
                seq: baseSeq + i,
                fee: 10
            });
        transactions.push(transaction);
    }

    const startTime = new Date();
    console.log(`Start at: ${startTime}`);

    for (let i = 0; i < numTransactions; i++) {
        await sdk.rpc.chain.sendSignedTransaction(transactions[i]);
        if (i % 1000 === 0) {
            console.log(i);
        }
    }

    const endTime = new Date();
    console.log(`End at: ${endTime}`);
    const throughput =
        (numTransactions * 1000.0) /
        (endTime.getTime() - startTime.getTime());
    console.log(
        `Elapsed time (ms): ${endTime.getTime() - startTime.getTime()}`
    );
    console.log(throughput);

    await node.clean();
})().catch(console.error);
