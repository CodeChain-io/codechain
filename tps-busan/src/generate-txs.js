const SDK = require("codechain-sdk");
const { generatePrivateKey } = require("codechain-primitives");
const sdk = new SDK({ server: "http://localhost:7070", networkId: "bw" });
const txSender = "bwcqxcr6rjs8545ywp5pgw98qcg2aruxftg7yaaywmr";
const fs = require("fs");

async function main() {
  const data = "hi";
  const content = JSON.stringify(data);
  const randomKey = generatePrivateKey();

  const seq = await sdk.rpc.chain.getSeq(txSender);

  const txs = [];

  const n = 1000;
  for (let i = 0; i < n; i += 1) {
    console.time("generate");
    const tx = await sdk.core.createStoreTransaction({
      content,
      secret: randomKey,
    });
    console.timeLog("generate", "createTx");

    const signedTx = await sdk.key.signTransaction(tx, {
      account: txSender,
      fee: 0,
      seq: seq + i,
    });
    const hexTx = signedTx.rlpBytes().toString("hex");
    txs.push(hexTx);
    console.timeLog("generate", "sign");
    console.timeEnd("generate");
  }

  fs.writeFileSync("./txs", JSON.stringify(txs));

  cnosole.log(`Generated ${n} txs`);
}

main().catch(console.error);
