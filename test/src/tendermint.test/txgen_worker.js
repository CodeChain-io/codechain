const {
    Worker,
    isMainThread,
    parentPort,
    workerData
} = require("worker_threads");
const { SDK } = require("codechain-sdk");
const { existsSync , readFileSync, writeFileSync} = require("fs")

async function main() {
    const { wname, secret, seqStart, seqEnd, filePrefix } = workerData;
    console.log(`${wname} // ${secret} // ${seqStart} // ${seqEnd} // ${filePrefix}`);
    const sdk = new SDK({
        server: `http://localhost:${8080}`,
        networkId: "tc"
    });

    const value = makeRandomH256();
            const accountId = sdk.util.getAccountIdFromPrivate(value);
            const recipient = sdk.core.classes.PlatformAddress.fromAccountId(accountId, {
                networkId: "tc"
            });

    let seqCount = seqStart;
    while (seqCount < seqEnd) {
        const transactions = [];
        let chunkStart = seqCount;
        for (let i = 0; i < 50000; i++) {

            
            if (seqCount % 1000 === 0) {
                console.log(`[tx prepared] Worker ${wname}: ${i}`);
            }
            const transaction = sdk.core
                    .createPayTransaction({
                        recipient,
                        quantity: 1
                    })
                    .sign({
                        secret: secret,
                        seq: seqCount,
                        fee: 10
                    });
            transactions.push(transaction.rlpBytes().toString("hex"));
            seqCount++;
        }
        writeFileSync(`/home/junha/Desktop/txsame/${filePrefix}_${chunkStart}_${seqCount}.json`, JSON.stringify(transactions));
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
