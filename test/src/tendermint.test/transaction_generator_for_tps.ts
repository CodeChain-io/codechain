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
    const server = process.env.SERVER!;
    const sdk = new SDK({
        server,
        networkId: "bc"
    });

    // const tempAccount = "bccqxsd0f56vwydcndezvhhc5klgwj4yrle4s22j075";
    // const tempAccount2 = "bccqxajak89n49y0aurknj2u286d2j8llh9s5h7s3lr";
    // const tempAccount3 = "bccqyanljaj2j9p7g2k0tckrux3vhncacpf2qe26htp";
    // const tempAccount4 = "bccqxhvq5jje9sk7qax9ng0p4acezl4lx9w3sp0ak5x";
    // const tempAccount5 = "bccq958cea4tty2f4xmwec3a7fpq7jwenatcsx7prwa";
    // const tempAccount6 = "bccqy6hagm93h4vnfp3es7kmgldzq022xph85h3dpeu";
    // const tempAccount7 = "bccq8yfcvpukrm4l3am787xcu7yrd96cvm9p552v82c";
    // const tempAccount8 = "bccqxe5283zt9l8jr68uzakqyca8vu2nuyrhqhn35fj";
    // const tempPrivate = "a056c66080e627a0fa32c0c9fa898d6c074f1af4d896f0a16cee27cb7e129a8b";
    // const tempPrivate2 = "cd4afd20958ef9865eaa7636558c740b07cb7bd35bb728153a5cbd11f67135d5";
    // const tempPrivate3 = "d195c6cd1a4ba63336dc98023fe699604c2fbb499b41dc102ab231faf2e0ecb8";
    // const tmpePrivate4 = "985aef7f06bcf05cef65a0eb430dc48f24c3c1f92efa89c980bf18d9f418f198";
    // const tempPrivate5 = "36a94840f1f4de303a1545a991d3c8df9df1b3006711b12097f9ba01d92f08a0";
    // const tempPrivate6 = "42d1e4d5d8722c5ce840c6e4847cd20fa51d9a651fbbda637735459a2739a67a";
    // const tempPrivate7 = "942406d575e353ea9d19e9cff51cc66dac7cda1c2be5e33eb9d06ea4314134b3";
    // const tempPrivate8 = "1d702feb00778bfad6e4e30425db46fd256c54e64b2ef0b56ed0b4c42a5c7cab";

    const tempAccount = process.env.RICH_ACCOUNT!;
    const tempPrivate = process.env.RICH_SECRET!;

    const beagleStakeHolder1 = "bccqxkppqfqwwl6vwge62qq22eh3xkmzqwvschr8thm";
    const baseSeq = await sdk.rpc.chain.getSeq(tempAccount);
    console.log("base seq fetched");

    for (let i = 0; ; i++) {
        const recipient = beagleStakeHolder1;
        const transaction = sdk.core
            .createPayTransaction({
                recipient,
                quantity: 1
            })
            .sign({
                secret: tempPrivate,
                seq: baseSeq + i,
                fee: 100
            });
        await sdk.rpc.chain.sendSignedTransaction(transaction);
        console.log(`${i}th transaction was sent`);
    }
})().catch(console.error);
