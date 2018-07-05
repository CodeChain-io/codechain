[JSON-RPC](http://www.jsonrpc.org/specification) is a stateless, light-weight remote procedure call (RPC) protocol. Primarily this specification defines several data structures and the rules around their processing. It is transport agnostic in that the concepts can be used within the same process, over sockets, over HTTP, or in many various message passing environments. It uses JSON ([RFC 4627](https://www.ietf.org/rfc/rfc4627.txt)) as data format.

# CLI options for JSON-RPC

 * `--no-jsonrpc`
   > Do not run jsonrpc.
 * `--jsonrpc-port <PORT>`
   > Listen for rpc connections on PORT. [default: 8080]

In the current version, it's only supported through HTTP.

# List of types

## block object

 - author: `string`
 - extraData: Array of any
 - hash: `string`
 - invoicesRoot: `string`
 - number: `number`
 - parcels: Array of `parcel object`
 - parcelsRoot: `string`
 - parentHash: `string`
 - score: `number`
 - seal: Array of `string`
 - stateRoot: `string`
 - timestamp: `number`

## parcel object

 - blockHash: `string`
 - blockNumber: `number`
 - fee: `string`
 - hash: `string`
 - networkId: `number`
 - nonce: `string`
 - parcelIndex: `number`
 - r: `string`
 - s: `string`
 - v: `number`
 - action: `action object`

## action objects

### ChangeShardState action object

 - action: "changeShardState"
 - transactions: Array of `transaction object`

### Payment action object

 - action: "payment"
 - receiver: `string`
 - value: `string`

### SetRegularKey action object

 - action: "setRegularKey"
 - key: `string`

## transaction object

 - type: `string` - "assetMint" | "assetTransfer"
 - data: `asset mint object` or `asset transfer object`

## asset scheme object

 - amount: `number`
 - metadata: `string`
 - registrar: `string` or `null`

## asset object

 - amount: `number`
 - asset_type: `string`
 - lock_script_hash: `string`
 - parameters: Array of `string`

# List of methods

 * [ping](#ping)
***
 * [chain_getBestBlockNumber](#chain_getbestblocknumber)
 * [chain_getBestBlockId](#chain_getbestblockid) (not implemented yet)
 * [chain_getBlockHash](#chain_getblockhash)
 * [chain_getBlockByHash](#chain_getblockbyhash)
 * [chain_sendSignedParcel](#chain_sendsignedparcel)
 * [chain_getParcel](#chain_getparcel)
 * [chain_getParcelInvoices](#chain_getparcelinvoices)
 * [chain_getTransactionInvoice](#chain_gettransactioninvoice)
 * [chain_getAssetScheme](#chain_getassetscheme)
 * [chain_getAsset](#chain_getasset)
 * [chain_getNonce](#chain_getnonce)
 * [chain_getBalance](#chain_getbalance)
 * [chain_getRegularKey](#chain_getregularkey)
 * [chain_getNumberOfShards](#chain_getnumberofshards)
 * [chain_getPendingParcels](#chain_getpendingparcels)
***
  * [miner_getWork](#miner_getwork)
  * [miner_submitWork](#miner_submitwork)
***
  * [net_shareSecret](#net_sharesecret)
  * [net_isConnected](#net_isconnected)
  * [net_disconnect](#net_disconnect)
***
 * [devel_getStateTrieKeys](#devel_getstatetriekeys)
 * [devel_getStateTrieValue](#devel_getstatetrievalue)


# Specification

## ping
Sends ping to check whether CodeChain's RPC server is responding or not

Params: No parameters

Return Type: `string`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "ping", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"pong","id":null}
```

## chain_getBestBlockNumber
Gets number of the best block.

Params: No parameters

Return Type: `number`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBestBlockNumber", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":1,"id":null}
```

## chain_getBestBlockId
(not implemented yet) Gets the number and the hash of the best block.

Params: No parameters

Return Type: { number: `number`, hash: `string` }

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBestBlockId", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":{"number":1,"hash":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"},"id":null}
```

## chain_getBlockHash
Gets the hash of the block with given number.

Params:
 1. n - `number`

Return Type: `null` or `string`

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockHash", "params": [1], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077","id":null}
```

## chain_getBlockByHash
Gets block with given hash.

Params:
 1. hash: `string`

Return Type: `null` or `block object`

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockByHash", "params": ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"], "id": null}' \
    localhost:8080
```

Response Example
```
{
    "id": null,
    "jsonrpc": "2.0",
    "result": {
        "author": "0x84137e7a75043bed32e4458a45da7549a8169b4d",
        "extraData": [],
        "hash": "0x49b5fda89dbfa92e9a744d3019790107757d189608e2cfe15e796825f4561959",
        "invoicesRoot": "0x45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0",
        "number": 1,
        "parcels": [
            {
                "action": {
                    "action": "changeShardState",
                    "transactions": []
                },
                "blockHash": "0x49b5fda89dbfa92e9a744d3019790107757d189608e2cfe15e796825f4561959",
                "blockNumber": 1,
                "fee": "0xa",
                "hash": "0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56",
                "networkId": 17,
                "nonce": "0x0",
                "parcelIndex": 0,
                "r": "0xab2f74e74344b0b24932c85e29a4039150ae0b9fab17398b7e138a70022fd09c",
                "s": "0x364dd6aeee95f45cbd6773c3edc6507d07505f7fbfb5d85ce128d19fa104d2a6",
                "v": 1
            }
        ],
        "parcelsRoot": "0x934b77fa1ff7f405127de3c63efd44b92dad7ee4ff923c9b77f06abebd4844a4",
        "parentHash": "0xc2338c8fd5a9b4ca5dd5dd12fc548e796bbb953ee6043afa14377037d0387e25",
        "score": "0x20000",
        "seal": [],
        "stateRoot": "0x223ac1b388a6f3a2e001482d328c7f6f3b8f0b8686d3988224870a8fed99c8b1",
        "timestamp": 1530694371
    }
}
```

## chain_sendSignedParcel
Sends signed parcel, returning its hash.

Params: 
 1. bytes: `string` - RLP encoded hex string of SignedParcel

Return Type: `string` - parcel hash

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_sendSignedParcel", "params": ["0xf849800a11c201c001a0ab2f74e74344b0b24932c85e29a4039150ae0b9fab17398b7e138a70022fd09ca0364dd6aeee95f45cbd6773c3edc6507d07505f7fbfb5d85ce128d19fa104d2a6"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56","id":null}
```

## chain_getParcel
Gets parcel with given hash.

Params:
 1. parcel hash - `string`

Return Type: `null` or `parcel object`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getParcel", "params": ["0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56"], "id": null}' \
    localhost:8080
```

Response Example
```
{
    "id": null,
    "jsonrpc": "2.0",
    "result": {
        "action": {
            "action": "changeShardState",
            "transactions": []
        },
        "blockHash": "0x49b5fda89dbfa92e9a744d3019790107757d189608e2cfe15e796825f4561959",
        "blockNumber": 1,
        "fee": "0xa",
        "hash": "0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56",
        "networkId": 17,
        "nonce": "0x0",
        "parcelIndex": 0,
        "r": "0xab2f74e74344b0b24932c85e29a4039150ae0b9fab17398b7e138a70022fd09c",
        "s": "0x364dd6aeee95f45cbd6773c3edc6507d07505f7fbfb5d85ce128d19fa104d2a6",
        "v": 1
    }
}
```

## chain_getParcelInvoice
Gets a parcel invoice with given hash.

Params:
 1. parcel hash - `string`

Return Type: `null` or Array of string `Success` or `Failed`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getParcelInvoice", "params": ["0xad708d48755ac36685280a45ec213941e21c41644c781bf2f487fd6c7e4b2ebb"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":["Success"],"id":null}
```

## chain_getTransactionInvoice
Gets transaction invoice with given hash

Params:
 1. transaction hash - `string`

Return Type: `null` or string `Success` or `Failed`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransactionInvoice", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"Success","id":null}
```

## chain_getAssetScheme
Gets asset scheme with given asset type.

Params:
 1. transaction hash of AssetMintTransaction - `string`

Return Type: `null` or `asset scheme object`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetScheme", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":{
  "amount":100,
  "metadata":"",
  "registrar":null
},"id":null}
```

## chain_getAsset
Gets asset with given asset type.

Params:
 1. transaction hash of AssetMintTransaction or AssetTransferTransaction - `string`
 2. index - `number`

Return Type: `null` or `asset object`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAsset", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":{
  "amount":100,
  "asset_type":"0x53000000000000002ec1193ecd52e2833ffc10b45bea1fda49f857e34db67c68",
  "lock_script_hash":"0x0000000000000000000000000000000000000000000000000000000000000000",
  "parameters":[]
},"id":null}
```

## chain_getNonce
Gets nonce of an account of given address, at state of given blockNumber.

Params:
 1. address: `string`
 2. block number: `number` or `null`

Return Type: `string`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNonce", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"0x54","id":null}
```

## chain_getBalance
Gets balance of an account of given address, at state of given blockNumber.

Params:
 1. address: `string`
 2. block number: `number` or `null`

Return Type: `string`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBalance", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"0xe8d4a50dd0","id":null}
```

## chain_getRegularKey
Gets the regular key of an account of given address, at state of given blockNumber.

Params:
 1. address: `string`
 2. block number: `number` or `null`

Return Type: `string` - 512-bit public key

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getRegularKey", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","id":null}
```

## chain_getNumberOfShards
Gets the number of shards, at state of given blockNumber.

Param:
1. block number: `number` or `null`

Return Type: `number` - the number of shards

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNumberOfShards", "params": [null], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":3,"id":null}
```


## chain_getPendingParcels
Gets parcels in the current parcel queue.

Params: No parameters

Return Type: Array of `parcel object`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getPendingParcels", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":[{
  "blockHash":null,
  "blockNumber":null,
  "fee":"0xa",
  "hash":"0x8ae3363ccdcc02d8d662d384deee34fb89d1202124e8065f0d6c84ab31e68d8a",
  "networkId":17,
  "nonce":"0x0",
  "parcelIndex":null,
  "r":"0x22605d6b9fb713d3a415e02eeed8b4a630e0d867c91bf7d9b7721f94159c0fe1",
  "s":"0x772f19f1c27f1db8b28289caa9e99ad756878fd56b2415c25cd47cc737f7e0c2",
  "transactions":[{
    "payment":{
      "nonce":"0x1",
      "receiver":"0xa6594b7196808d161b6fb137e781abbc251385d9",
      "sender":"0xa6594b7196808d161b6fb137e781abbc251385d9",
      "value":"0x0"
    }
  }],
  "v":0
}],"id":null}
```

## miner_getWork
Returns the hash of the current block, the score and the block number.

Params: No parameters

Return Type: `work object`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_getWork", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":{
  "blockHash": "0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
  "score": 100,
  "blockNumber": 30,
},"id":null}
```

## miner_submitWork
Used for submitting a proof-of-work solution.

Params:
 1. nonce: `string`
 1. powHash: `string`

Return Type: `bool`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_submitWork", "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", "0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":true,"id":6}
```

## net_shareSecret
Share secret to given address.

Params:
 1. secret: `string`
 3. address: `string`
 4. port: `number`

Return Type: null

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_shareSecret", "params": ['0x8ae3363ccdcc02d8d662d384deefb89d', 'codechain.example.com', '3485'], "id": 5}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":null,"id":5}
```

## net_isConnected
Check whether the connection is established

Params:
 1. address: `string`
 1. port: `number`

Return Type: bool

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_isConnected", "params": ['codechain.example.com', '3485'], "id": 6}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":true,"id":6}
```

## net_disconnect
Disconnect the connection to the given address

Params:
 1. address: `string`
 1. port: `number`

Return Type: `bool`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_disconnect", "params": ['codechain.example.com', '3485'], "id": 6}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":true,"id":6}
```

## devel_getStateTrieKeys
Gets keys of the state trie with given offset and limit.

Params:
 1. offset: `number`
 2. limit: `number`

Return Type: Array of `string` with maximum length _limit_

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieKeys", "params": [0, 1], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":["0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"],"id":null}
```

## devel_getStateTrieValue
Gets the value of the state trie with given key.

Params: 
 1. key: `string`

Return Type: Array of `string` - each string is RLP encoded

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieValue", "params": ["0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"], "id": null}' \
    localhost:8080
```

Response Example
```
{"jsonrpc":"2.0","result":["0x20d560025f3a1c6675cb32384355ae05b224a3473ae17d3d15b6aa164af7d717","0xf84541a053000000000000002ab33f741ba153ff1ffdf1107845828637c864d5360e4932a00000000000000000000000000000000000000000000000000000000000000000c06f"],"id":null}
```
