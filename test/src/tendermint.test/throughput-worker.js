const {
    Worker,
    isMainThread,
    parentPort,
    workerData
} = require("worker_threads");
const { SDK } = require("codechain-sdk");

async function main() {
    const { index, port, validatorSecrets } = workerData;

    const sdk = new SDK({
        server: `http://localhost:${port}`,
        networkId: "tc"
    });

    for (let i = 0; i < Number.MAX_SAFE_INTEGER; i++) {
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
                secret: validatorSecrets[index],
                seq: i,
                fee: 10
            });
        await sdk.rpc.chain.sendSignedTransaction(transaction);
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
