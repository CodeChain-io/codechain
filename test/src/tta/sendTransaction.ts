import { faucetSecret, faucetAddress } from "../helper/constants";
import { makeRandomH256 } from "../helper/random";
import { SDK } from "codechain-sdk";

const NETWORK_ID = "tc";

async function main() {
    const sdk = new SDK({
        server: "http://127.0.0.1:2487",
        networkId: NETWORK_ID
    });

    const quantity = 1;
    const seq = await sdk.rpc.chain.getSeq(faucetAddress);
    const recipient = createRandomAddress();
    const transaction = sdk.core
        .createPayTransaction({
            recipient,
            quantity
        })
        .sign({
            secret: faucetSecret,
            seq,
            fee: 10
        });
    const txHash = await sdk.rpc.chain.sendSignedTransaction(transaction);

    console.log(`Send ${quantity} CCC to ${recipient.toString()}`);
    console.log(`Transaction hash: ${txHash.toString()}`);
}

main().catch(console.error);

function createRandomAddress() {
    const value = makeRandomH256();
    const accountId = SDK.util.getAccountIdFromPrivate(value);
    return SDK.Core.classes.PlatformAddress.fromAccountId(accountId, {
        networkId: NETWORK_ID
    });
}
