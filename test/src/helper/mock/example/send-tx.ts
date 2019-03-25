import { Mock } from "..";

const SDK = require("codechain-sdk");

async function sendTransaction() {
    const mock = new Mock("0.0.0.0", 3485, "tc");
    mock.setLog();
    await mock.establish();

    const sdk = new SDK({
        server: process.env.CODECHAIN_RPC_HTTP || "http://localhost:8080",
        networkId: process.env.CODECHAIN_NETWORK_ID || "tc"
    });
    const ACCOUNT_SECRET =
        process.env.ACCOUNT_SECRET || "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
    const unsigned = sdk.core.createPayTransaction({
        recipient: "tccqruq09sfgax77nj4gukjcuq69uzeyv0jcs7vzngg",
        amount: 10000
    });
    const signed = unsigned.sign({
        secret: ACCOUNT_SECRET,
        fee: 10,
        nonce: 0
    });

    await mock.sendEncodedTransaction([signed.toEncodeObject()]);

    await mock.end();
}
sendTransaction();
