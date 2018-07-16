[JSON-RPC](http://www.jsonrpc.org/specification) is a stateless, light-weight remote procedure call (RPC) protocol. Primarily this specification defines several data structures and the rules around their processing. It is transport agnostic, meaning that the concepts can be used within the same process, over sockets, over HTTP, or in many various message passing environments. It uses JSON ([RFC 4627](https://www.ietf.org/rfc/rfc4627.txt)) as data format.

# CLI options for JSON-RPC

 * `--no-jsonrpc`
   > Do not run jsonrpc.
 * `--jsonrpc-port <PORT>`
   > Listen for rpc connections on PORT. [default: 8080]

In the current version, it's only supported through HTTP.

# List of types

## H160, H256, H512, ...

A XXX-bit hexadecimal string. (e.g. H160: 160-bit hexadecimal string)

## U128, U256, U512, ...

A hexadecimal string for XXX-bit unsigned integer

## BlockObject

 - author: `H160`
 - extraData: `any[]`
 - hash: `H256`
 - invoicesRoot: `H256`
 - number: `number`
 - parcels: `ParcelObject[]`
 - parcelsRoot: `H256`
 - parentHash: `H256`
 - score: `number`
 - seal: `string[]`
 - stateRoot: `H256`
 - timestamp: `number`

## ParcelObject

 - blockHash: `H256`
 - blockNumber: `number`
 - fee: `U256`
 - hash: `H256`
 - networkId: `number`
 - nonce: `U256`
 - parcelIndex: `number`
 - sig: `H520` for ECDSA signature | `H512` for Schnorr signature
 - action: `ActionObject`

## ActionObjects

### ChangeShardState ActionObject

 - action: "changeShardState"
 - transactions: `TransactionObject[]`

### Payment ActionObject

 - action: "payment"
 - receiver: `H160`
 - amount: `U256`

### SetRegularKey ActionObject

 - action: "setRegularKey"
 - key: `H512`

## TransactionObject

 - type: "assetMint" | "assetTransfer"
 - data: `AssetMintObject` | `AssetTransferObject`

## AssetSchemeObject

 - amount: `number`
 - metadata: `string`
 - registrar: `H160` | `null`

## AssetObject

 - amount: `number`
 - asset_type: `H256`
 - lock_script_hash: `H256`
 - parameters: `hexadecimal string[]`

# List of methods

 * [ping](#ping)
 * [version](#version)
***
 * [chain_getBestBlockNumber](#chain_getbestblocknumber)
 * [chain_getBestBlockId](#chain_getbestblockid)
 * [chain_getBlockHash](#chain_getblockhash)
 * [chain_getBlockByHash](#chain_getblockbyhash)
 * [chain_sendSignedParcel](#chain_sendsignedparcel)
 * [chain_getParcel](#chain_getparcel)
 * [chain_getParcelInvoice](#chain_getparcelinvoice)
 * [chain_getTransactionInvoice](#chain_gettransactioninvoice)
 * [chain_getAssetScheme](#chain_getassetscheme)
 * [chain_getAsset](#chain_getasset)
 * [chain_getNonce](#chain_getnonce)
 * [chain_getBalance](#chain_getbalance)
 * [chain_getRegularKey](#chain_getregularkey)
 * [chain_getNumberOfShards](#chain_getnumberofshards)
 * [chain_getShardRoot](#chain_getshardroot)
 * [chain_getPendingParcels](#chain_getpendingparcels)
 * [chain_getCoinbase](#chain_getcoinbase)
***
  * [miner_getWork](#miner_getwork)
  * [miner_submitWork](#miner_submitwork)
***
  * [net_shareSecret](#net_sharesecret)
  * [net_connect](#net_connect)
  * [net_isConnected](#net_isconnected)
  * [net_disconnect](#net_disconnect)
  * [net_getPeerCount](#net_getPeerCount)
  * [net_getPort](#net_getPort)
  * [net_getNetworkId](#net_getNetworkId)
***
 * [devel_getStateTrieKeys](#devel_getstatetriekeys)
 * [devel_getStateTrieValue](#devel_getstatetrievalue)


# Specification

## ping
Sends ping to check whether CodeChain's RPC server is responding or not

Params: No parameters

Return Type: `string` - "pong"

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "ping", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"pong",
  "id":null
}
```

## version
Gets the version of CodeChain

Params: No parameters

Return Type: `string` - e.g. 0.1.0

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "version", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0.1.0",
  "id":null
}
```

## chain_getBestBlockNumber
Gets the number of the best block.

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
{
  "jsonrpc":"2.0",
  "result":1,
  "id":null
}
```

## chain_getBestBlockId
Gets the number and the hash of the best block.

Params: No parameters

Return Type: { number: `number`, hash: `H256` }

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBestBlockId", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "hash":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
    "number":1
  },
  "id":null
}
```

## chain_getBlockHash
Gets the hash of the block with given number.

Params:
 1. n - `number`

Return Type: `null` | `H256`

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockHash", "params": [1], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
  "id":null
}
```

## chain_getBlockByHash
Gets the block with the given hash.

Params:
 1. hash: `H256`

Return Type: `null` | `BlockObject`

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
  "id":null,
  "jsonrpc":"2.0",
  "result":{
    "author":"0x84137e7a75043bed32e4458a45da7549a8169b4d",
    "extraData":[

    ],
    "hash":"0x49b5fda89dbfa92e9a744d3019790107757d189608e2cfe15e796825f4561959",
    "invoicesRoot":"0x45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0",
    "number":1,
    "parcels":[
      {
        "action":{
          "action":"changeShardState",
          "transactions":[

          ]
        },
        "blockHash":"0x49b5fda89dbfa92e9a744d3019790107757d189608e2cfe15e796825f4561959",
        "blockNumber":1,
        "fee":"0xa",
        "hash":"0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56",
        "networkId":17,
        "nonce":"0x0",
        "parcelIndex":0,
        "r":"0xab2f74e74344b0b24932c85e29a4039150ae0b9fab17398b7e138a70022fd09c",
        "s":"0x364dd6aeee95f45cbd6773c3edc6507d07505f7fbfb5d85ce128d19fa104d2a6",
        "v":1
      }
    ],
    "parcelsRoot":"0x934b77fa1ff7f405127de3c63efd44b92dad7ee4ff923c9b77f06abebd4844a4",
    "parentHash":"0xc2338c8fd5a9b4ca5dd5dd12fc548e796bbb953ee6043afa14377037d0387e25",
    "score":"0x20000",
    "seal":[

    ],
    "stateRoot":"0x223ac1b388a6f3a2e001482d328c7f6f3b8f0b8686d3988224870a8fed99c8b1",
    "timestamp":1530694371
  }
}
```

## chain_sendSignedParcel
Sends a signed parcel, returning its hash.

Params: 
 1. bytes: `hexadecimal string` - RLP encoded hex string of SignedParcel

Return Type: `H256` - parcel hash

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_sendSignedParcel", "params": ["0xf849800a11c201c001a0ab2f74e74344b0b24932c85e29a4039150ae0b9fab17398b7e138a70022fd09ca0364dd6aeee95f45cbd6773c3edc6507d07505f7fbfb5d85ce128d19fa104d2a6"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x20dced7a95e82cf165bbb7ef111bfda24b664e3c3ffd5a255e970300eea5ec56",
  "id":null
}
```

## chain_getParcel
Gets a parcel with the given hash.

Params:
 1. parcel hash - `H256`

Return Type: `null` or `ParcelObject`

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
Gets a parcel invoice with the given hash.

Params:
 1. parcel hash - `H256`

Return Type: `null` | string[]. The string either "Success" or "Failed"

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getParcelInvoice", "params": ["0xad708d48755ac36685280a45ec213941e21c41644c781bf2f487fd6c7e4b2ebb"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "Success"
  ],
  "id":null
}
```

## chain_getTransactionInvoice
Gets a transaction invoice with the given hash.

Params:
 1. transaction hash - `H256`

Return Type: `null` | "Success" | "Failed"

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransactionInvoice", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"Success",
  "id":null
}
```

## chain_getAssetScheme
Gets an asset scheme with the given asset type.

Params:
 1. transaction hash of AssetMintTransaction - `H256`

Return Type: `null` | `AssetSchemeObject`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetScheme", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "amount":100,
    "metadata":"",
    "registrar":null
  },
  "id":null
}
```

## chain_getAsset
Gets an asset with the given asset type.

Params:
 1. transaction hash - `H256`
 2. index - `number`

Return Type: `null` | `AssetObject`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAsset", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "amount":100,
    "asset_type":"0x53000000000000002ec1193ecd52e2833ffc10b45bea1fda49f857e34db67c68",
    "lock_script_hash":"0x0000000000000000000000000000000000000000000000000000000000000000",
    "parameters":[

    ]
  },
  "id":null
}
```

## chain_getNonce
Gets a nonce of an account of the given address, at state of the given blockNumber.

Params:
 1. address: `H160`
 2. block number: `number` | `null`

Return Type: `U256`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNonce", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x54",
  "id":null
}
```

## chain_getBalance
Gets a balance of an account of the given address, at the state of the given blockNumber.

Params:
 1. address: `H160`
 2. block number: `number` | `null`

Return Type: `U256`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBalance", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xe8d4a50dd0",
  "id":null
}
```

## chain_getRegularKey
Gets the regular key of an account of the given address, at the state of the given blockNumber.

Params:
 1. address: `H160`
 2. block number: `number` | `null`

Return Type: `H512` - 512-bit public key

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getRegularKey", "params": ["0xa6594b7196808d161b6fb137e781abbc251385d9", null], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
  "id":null
}
```

## chain_getNumberOfShards
Gets the number of shards, at the state of the given blockNumber.

Param:
1. block number: `number` | `null`

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
{
  "jsonrpc":"2.0",
  "result":3,
  "id":null
}
```

## chain_getShardRoot
Gets the root of shard, at the state of the given blockNumber.

Param:
1. shard id: `number`
1. block number: `number` | `null`

Return Type: `null` | `H256` - the root of shard

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getShardRoot", "params": [1, null], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xf3841adc1615bfeabb801dda23585c1722b80d810df084a5f2198e92285d4bfd",
  "id":null
}
```


## chain_getPendingParcels
Gets parcels in the current parcel queue.

Params: No parameters

Return Type: `ParcelObject[]`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getPendingParcels", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    {
      "blockHash":null,
      "blockNumber":null,
      "fee":"0xa",
      "hash":"0x8ae3363ccdcc02d8d662d384deee34fb89d1202124e8065f0d6c84ab31e68d8a",
      "networkId":17,
      "nonce":"0x0",
      "parcelIndex":null,
      "r":"0x22605d6b9fb713d3a415e02eeed8b4a630e0d867c91bf7d9b7721f94159c0fe1",
      "s":"0x772f19f1c27f1db8b28289caa9e99ad756878fd56b2415c25cd47cc737f7e0c2",
      "transactions":[
        {
          "payment":{
            "nonce":"0x1",
            "receiver":"0xa6594b7196808d161b6fb137e781abbc251385d9",
            "sender":"0xa6594b7196808d161b6fb137e781abbc251385d9",
            "value":"0x0"
          }
        }
      ],
      "v":0
    }
  ],
  "id":null
}
```

## chain_getCoinbase
(not implemented) Gets coinbase's account id.

Params: No parameters

Return Type: `H160` | `null`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getCoinbase", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xa6594b7196808d161b6fb137e781abbc251385d9",
  "id":null
}
```

## miner_getWork
Returns the hash of the current block, score and block number.

Params: No parameters

Return Type: `WorkObject`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_getWork", "params": [], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "powHash":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
    "target":100
  },
  "id":null
}
```

## miner_submitWork
Used for submitting a proof-of-work solution.

Params:
 1. powHash: `string`
 1. seal: `string[]`

Return Type: `bool`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_submitWork", "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"]], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":true,
  "id":6
}
```

## net_shareSecret
Share secret to the given address.

Params:
 1. secret: `string`
 3. address: `string`
 4. port: `number`

Return Type: null

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_shareSecret", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", "192.168.0.3", 3485], "id": 5}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":5
}
```

## net_connect
Connect to the given address.

Params:
 1. address: `string`
 1. port: `number`

Return Type: null

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_connect", "params": ["192.168.0.3", 3485], "id": 5}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":5
}
```

## net_isConnected
Check whether the connection is established.

Params:
 1. address: `string`
 1. port: `number`

Return Type: bool

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_isConnected", "params": ["192.168.0.3", "3485"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":true,
  "id":6
}
```

## net_disconnect
Disconnect the connection from the given address.

Params:
 1. address: `string`
 1. port: `number`

Return Type: `bool`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_disconnect", "params": ["192.168.0.3", "3485"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":true,
  "id":6
}
```

## net_getPeerCount
(not implemented) Return the count of peers which the client is connected to.

Params: No parameters

Return Type: `number`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getPeerCount", "params": [], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result": 34,
  "id":6
}
```


## net_getPort
(not implemented) Return the port number on which the client is listening for peers.

Params: No parameters

Return Type: `number`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getPort", "params": [], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result": 3485,
  "id":6
}
```

## net_getNetworkId
(not implemented) Return the nework id that is used in this chain.

Params: No parameters

Return Type: `number`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getNetworkId", "params": [], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result": 17,
  "id":6
}
```

## devel_getStateTrieKeys
Gets keys of the state trie with the given offset and limit.

Params:
 1. offset: `number`
 2. limit: `number`

Return Type: `string[]` with maximum length _limit_

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieKeys", "params": [0, 1], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"
  ],
  "id":null
}
```

## devel_getStateTrieValue
Gets the value of the state trie with the given key.

Params: 
 1. key: `string`

Return Type: `string[]` - each string is RLP encoded

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieValue", "params": ["0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "0x20d560025f3a1c6675cb32384355ae05b224a3473ae17d3d15b6aa164af7d717",
    "0xf84541a053000000000000002ab33f741ba153ff1ffdf1107845828637c864d5360e4932a00000000000000000000000000000000000000000000000000000000000000000c06f"
  ],
  "id":null
}
```
