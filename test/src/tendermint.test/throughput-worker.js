const {
    Worker,
    isMainThread,
    parentPort,
    workerData
} = require("worker_threads");
const { SDK } = require("codechain-sdk");

const faucetSecret =
    "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
const faucetAccointId = SDK.util.getAccountIdFromPrivate(faucetSecret); // 6fe64ffa3a46c074226457c90ccb32dc06ccced1
const faucetAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    faucetAccointId,
    { networkId: "tc" }
); // tccq9h7vnl68frvqapzv3tujrxtxtwqdnxw6yamrrgd

let globalTxs = [];

async function main() {
    generateTxs().catch(console.error);
    sendTxs().catch(console.error);
}

async function generateTxs() {
    const { index, port, validatorSecrets } = workerData;
    const sdk = new SDK({
        server: `http://localhost:${port}`,
        networkId: "tc"
    });

    for (var i = 0;  i < Number.MAX_SAFE_INTEGER; i++) {
        const value = makeRandomH256();
        const accountId = sdk.util.getAccountIdFromPrivate(value);
        const transaction = sdk.core
            .createPayTransaction({
                recipient: faucetAddress,
                quantity: 1
            })
            .sign({
                secret: validatorSecrets[index],
                seq: i,
                fee: 10
            });
        globalTxs.push("0x" + transaction.rlpBytes().toString("hex"));
        await wait(1);
    }
}

async function sendTxs() {
    const { port } = workerData;

    const sdk = new SDK({
        server: `http://localhost:${port}`,
        networkId: "tc"
    });

    for (let i = 0; i < Number.MAX_SAFE_INTEGER; i++) {
        if (globalTxs.length > 0) {
            const txs = globalTxs;
            globalTxs = [];
            await sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [txs]);
        } else {
            await wait(100);
        }
    }
}

main().catch(console.error);

function makeRandomH256() {
    let text = "";
    const possible = "0123456789abcdef";
    for (let i = 0; i < 64; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}

async function wait(duration) {
    await new Promise(resolve => setTimeout(() => resolve(), duration));
}

