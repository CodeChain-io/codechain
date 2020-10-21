const SDK = require("codechain-sdk");
const { generatePrivateKey } = require("codechain-primitives");
const sdk = new SDK({ server: "http://localhost:7070", networkId: "bw" });
const txSender = "bwcqxcr6rjs8545ywp5pgw98qcg2aruxftg7yaaywmr";
const fs = require("fs");

const consoleKey = "load and send tx";
async function main() {
  let txs = JSON.parse(fs.readFileSync("./txs.json"));

  console.time(consoleKey);

  let lastHash = "";
  for (let i = 0; i < txs.length; i += 1) {
    lastHash = await sdk.rpc.sendRpcRequest("mempool_sendSignedTransaction", [
      "0x" + txs[i],
    ]);
  }

  console.timeLog(consoleKey, `Sent ${txs.length} txs`);

  while (true) {
    const contain = await sdk.rpc.chain.containsTransaction(lastHash);
    if (contain) {
      break;
    }
    await sleep(100);
  }

  console.timeLog(consoleKey, "All txs mined");

  console.timeEnd(consoleKey);
}

main().catch(console.error);

async function sleep(millis) {
  return new Promise((resolve) => {
    setTimeout(resolve, millis);
  });
}
