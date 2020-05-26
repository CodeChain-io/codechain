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

import {
    generatePrivateKey,
    getAccountIdFromPublic,
    getPublicFromPrivate,
    PlatformAddress
} from "codechain-primitives";
import { SDK } from "codechain-sdk";

(async () => {
    const networkId = "bc";
    const sdk = new SDK({
        server: "http://192.168.1.101:2487",
        networkId
    });

    const tempPrivate = generatePrivateKey();
    const tempAccount = PlatformAddress.fromAccountId(
        getAccountIdFromPublic(getPublicFromPrivate(tempPrivate)),
        {
            networkId
        }
    );
    console.log(
        `The private key for a new account for tps measurement is ${tempPrivate}` +
            ` and the address is ${tempAccount} `
    );

    const beagleStakeHolder0 = "bccqypclxxrlr8f9n75dt6ayasvkdkxx6k3qgedauzj";
    const beagleStakeHolder0Password = "bN42rTEDrhFNyAWZ";
    const baseSeq = await sdk.rpc.chain.getSeq(beagleStakeHolder0);

    {
        const recipient = tempAccount;
        const transaction = sdk.core.createPayTransaction({
            recipient,
            quantity: 100000000
        });
        await sdk.rpc.chain.sendTransaction(transaction, {
            account: beagleStakeHolder0,
            passphrase: beagleStakeHolder0Password,
            seq: baseSeq,
            fee: 100
        });
    }
})().catch(console.error);
