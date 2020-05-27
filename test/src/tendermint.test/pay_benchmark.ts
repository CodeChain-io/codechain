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
(async () => {
    const sdk = new SDK({
        server: "http://localhost:8080",
        networkId: "tc"
    });

    const bundleSize = 10000;

    const transactions = [];
    const startTime = new Date();
    console.log(`Start at: ${startTime}`);
    for (let i = 0; i < bundleSize; i++) {
        const transaction = sdk.core.createPayTransaction({
           recipient: "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqpqc2ul2h",
           quantity: 1, 
        }).sign({
            secret: "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd",
            seq: i,
            fee: 100,
        });
        transactions.push(transaction)
    }
    const endTime = new Date();
    console.log(`End at: ${endTime}`);
    const throughput =
        (bundleSize * 1000.0) / (endTime.getTime() - startTime.getTime());
    console.log(
        `Elapsed time (ms): ${endTime.getTime() - startTime.getTime()}`
    );
    console.log(`throughput: ${throughput}`);
})().catch(console.error);
