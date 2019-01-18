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

import { faucetAddress, faucetSecret } from "../helper/constants";
import { wait } from "../helper/promise";
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";

(async () => {
    const numTransactions = parseInt(process.env.TEST_NUM_TXS || "10000", 10);
    const rpcPort = parseInt(process.env.TEST_RPC_PORT || "8080", 10);

    const node = new CodeChain({
        argv: ["--reseal-min-period", "0"],
        rpcPort
    });

    const transactions = [];
    const baseSeq = await node.sdk.rpc.chain.getSeq(faucetAddress);

    for (let i = 0; i < numTransactions; i++) {
        const value = makeRandomH256();
        const accountId = node.sdk.util.getAccountIdFromPrivate(value);
        const recipient = node.sdk.core.classes.PlatformAddress.fromAccountId(
            accountId,
            { networkId: "tc" }
        );
        const transaciton = node.sdk.core
            .createPayTransaction({
                recipient,
                quantity: 1
            })
            .sign({
                secret: faucetSecret,
                seq: baseSeq + i,
                fee: 10
            });
        transactions.push(transaciton);
    }

    for (let i = numTransactions - 1; i > 0; i--) {
        await node.sdk.rpc.chain.sendSignedTransaction(transactions[i]);
    }
    const startTime = new Date();
    console.log(`Start at: ${startTime}`);
    await node.sdk.rpc.chain.sendSignedTransaction(transactions[0]);

    while (true) {
        const invoice = await node.sdk.rpc.chain.getInvoice(
            transactions[numTransactions - 1].hash()
        );
        console.log(`Node invoice: ${invoice}`);
        if (invoice !== null && invoice.success) {
            break;
        }

        await wait(500);
    }
    const endTime = new Date();
    console.log(`End at: ${endTime}`);
    const tps =
        (numTransactions * 1000.0) / (endTime.getTime() - startTime.getTime());
    console.log(
        `Elapsed time (ms): ${endTime.getTime() - startTime.getTime()}`
    );
    console.log(`TPS: ${tps}`);
})().catch(console.error);
