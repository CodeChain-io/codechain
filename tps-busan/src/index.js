const SDK = require("codechain-sdk");
const { generatePrivateKey } = require("codechain-primitives");
const sdk = new SDK({ server: "http://localhost:7070", networkId: "bw" });
const txSender = "bwcqxcr6rjs8545ywp5pgw98qcg2aruxftg7yaaywmr";

async function main() {
  const data = "hi";
  const content = JSON.stringify(data);
  const randomKey = generatePrivateKey();

  const seq = await sdk.rpc.chain.getSeq(txSender);

  for (let i = 0; i < 100; i += 1) {
    console.time("send");
    const tx = await sdk.core.createStoreTransaction({
      content,
      secret: randomKey,
    });
    console.timeLog("send", "createTx");

    const signedTx = await sdk.key.signTransaction(tx, {
      account: txSender,
      fee: 0,
      seq: seq + i,
    });
    console.timeLog("send", "sign");
    await sdk.rpc.chain.sendSignedTransaction(signedTx);
    console.timeLog("send", "send");
    console.timeEnd("send");
  }

  console.log("Sent 100 transactions");
}

main().catch(console.error);
