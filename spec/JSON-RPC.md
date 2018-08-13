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

## PlatformAddress

A base32 string that starts with "ccc" or "tcc". See [the specification](https://github.com/CodeChain-io/codechain/blob/master/spec/CodeChain-Address.md#1-platform-account-address-format).

## Block

 - author: `PlatformAddress`
 - extraData: `any[]`
 - hash: `H256`
 - invoicesRoot: `H256`
 - number: `number`
 - parcels: `Parcel[]`
 - parcelsRoot: `H256`
 - parentHash: `H256`
 - score: `number`
 - seal: `string[]`
 - stateRoot: `H256`
 - timestamp: `number`

## Parcel

 - blockHash: `H256`
 - blockNumber: `number`
 - fee: `U256`
 - hash: `H256`
 - networkId: `number`
 - nonce: `U256`
 - parcelIndex: `number`
 - sig: `Signature`
 - action: `Action`

## Actions

### ChangeShardState Action

 - action: "changeShardState"
 - transactions: `Transaction[]`
 - changes: `ChangeShard[]`

### Payment Action

 - action: "payment"
 - receiver: `PlatformAddress`
 - amount: `U256`

### SetRegularKey Action

 - action: "setRegularKey"
 - key: `H512`

### ChangeShardOwners Action

 - action: "changeShardOwners"
 - shard_id: `number`
 - owners: `PlatformAddress[]`

### ChangeShardUsers Action

 - action: "changeShardUsers"
 - shard_id: `number`
 - users: `PlatformAddress[]`

## Transaction

 - type: "createWorld" | "setWorldOwners" | "setWorldUsers"| "assetMint" | "assetTransfer"
 - data: `CreateWorld` | `SetWorldOwners` | `SetWorldUsers`| `AssetMint` | `AssetTransfer`

## AssetScheme

 - amount: `number`
 - metadata: `string`
 - registrar: `PlatformAddress` | `null`

## Asset

 - amount: `number`
 - asset_type: `H256`
 - lock_script_hash: `H256`
 - parameters: `hexadecimal string[]`

## ChangeShard
- shard_id: `number`
- pre_root: `H256`
- post_root: `H256`

## Signature
`H520` for ECDSA signature | `H512` for Schnorr signature

# Error codes

| Code | Message | Description |
|---|---|---|
| -32002 | `No Author` | No author is configured |
| -32004 | `No Work Required` | No work is required |
| -32005 | `No Work Found` | No work is found |
| -32009 | `Invalid RLP` | Failed to decode the RLP string |
| -32011 | `KVDB Error` | Failed to access the state (Internal error of CodeChain) |
| -32010 | `Execution Failed` | Failed to execute the transactions |
| -32030 | `Verification Failed` | The signature is invalid or the network id does not match |
| -32031 | `Already Imported` | The same parcel is already imported |
| -32032 | `Not Enough Balance` | The signer's balance is insufficient |
| -32033 | `Too Low Fee` | The fee is lower than the minimum required |
| -32034 | `Too Cheap to Replace` | The fee is lower than the existing one in the queue |
| -32035 | `Invalid Nonce` | The signer's nonce is invalid to import |
| -32040 | `Keystore Error` | Failed to access the key store (Internal error of CodeChain) |
| -32041 | `Key Error` | The key is invalid |
| -32042 | `Already Exists` | The account already exists |
| -32043 | `Wrong Password` | The password does not match |
| -32044 | `No Such Account` | There is no such account in the key store |
| -32602 | `Invalid Params` | At least one of the parameters is invalid |

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
 * [chain_getTransaction](#chain_gettransaction)
 * [chain_getTransactionInvoice](#chain_gettransactioninvoice)
 * [chain_getAssetSchemeByHash](#chain_getassetschemebyhash)
 * [chain_getAssetSchemeByType](#chain_getassetschemebytype)
 * [chain_getAsset](#chain_getasset)
 * [chain_isAssetSpent](#chain_isassetspent)
 * [chain_getNonce](#chain_getnonce)
 * [chain_getBalance](#chain_getbalance)
 * [chain_getRegularKey](#chain_getregularkey)
 * [chain_getNumberOfShards](#chain_getnumberofshards)
 * [chain_getShardRoot](#chain_getshardroot)
 * [chain_getPendingParcels](#chain_getpendingparcels)
 * [chain_getCoinbase](#chain_getcoinbase)
 * [chain_executeTransactions](#chain_executetransactions)
 * [chain_getNetworkId](#chain_getNetworkId)
***
  * [miner_getWork](#miner_getwork)
  * [miner_submitWork](#miner_submitwork)
***
  * [net_shareSecret](#net_sharesecret)
  * [net_connect](#net_connect)
  * [net_isConnected](#net_isconnected)
  * [net_disconnect](#net_disconnect)
  * [net_getPeerCount](#net_getpeercount)
  * [net_getPort](#net_getport)
***
 * [account_getList](#account_getlist)
 * [account_create](#account_create)
 * [account_importRaw](#account_importraw)
 * [account_remove](#account_remove)
 * [account_sign](#account_sign)
 * [account_changePassword](#account_changepassword)
***
 * [shardValidator_registerAction](#shardvalidator_registeraction)
 * [shardValidator_getSignatures](#shardvalidator_getsignatures)
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

Errors: `Invalid Params`

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

Return Type: `null` | `Block`

Errors: `Invalid Params`

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockByHash", "params": ["0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "author":"cccqzzpxln6w5zrhmfju3zc53w6w4y6s95mf5lfasfn",
    "extraData":[

    ],
    "hash":"0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
    "invoicesRoot":"0x3a14d04383882243a684a6b0e779905f7883b12b5fb3ebf738facfcd2095b77a",
    "number":5,
    "parcels":[
      {
        "action":{
          "action":"payment",
          "amount":"0xa",
          "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"
        },
        "blockHash":"0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
        "blockNumber":5,
        "fee":"0xa",
        "hash":"0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
        "networkId":17,
        "nonce":"0x4",
        "parcelIndex":0,
        "sig":"0x291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"
      }
    ],
    "parcelsRoot":"0x0270d11d2bd21a0ec8e78d1c4e918103d7c4b02fdf734051231cb9eea90ae88e",
    "parentHash":"0xddf9fece0c6dee067a409e73a299bca21cec2d8300dff45739a5b76c680f378d",
    "score":"0x20000",
    "seal":[

    ],
    "stateRoot":"0x898961f82629a47ade064f15d3902a455379cb082e62d3995f21050df3f553dc",
    "timestamp":1531583888
  }
  "id":null
}
```

## chain_sendSignedParcel
Sends a signed parcel, returning its hash.

Params: 
 1. bytes: `hexadecimal string` - RLP encoded hex string of SignedParcel

Return Type: `H256` - parcel hash

Errors: `Invalid RLP`, `Verification Failed`, `Already Imported`, `Not Enough Balance`, `Too Low Fee`, `Too Cheap to Replace`, `Invalid Nonce`, `Invalid Params`

Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_sendSignedParcel", "params": ["0xf85e040a11d70294a6594b7196808d161b6fb137e781abbc251385d90ab841291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
  "id":null
}
```

## chain_getParcel
Gets a parcel with the given hash.

Params:
 1. parcel hash - `H256`

Return Type: `null` or `Parcel`

Errors: `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getParcel", "params": ["0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6"], "id": null}' \
    localhost:8080
```

Response Example
```
{
    "jsonrpc": "2.0",
    "result": {
        "action": {
          "action":"payment",
          "amount":"0xa",
          "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"
        },
        "blockHash": "0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
        "blockNumber": 5,
        "fee": "0xa",
        "hash": "0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
        "networkId": 17,
        "nonce": "0x4",
        "parcelIndex": 0,
        "sig":"0x291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"
    }
    "id": null,
}
```

## chain_getParcelInvoice
Gets a parcel invoice with the given hash.

Params:
 1. parcel hash - `H256`

Return Type: `null` | string[]. The string either "Success" or "Failed"

Errors: `Invalid Params`

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

## chain_getTransaction
Gets a transaction with the given hash.

Params:
 1. transaction hash - `H256`

Return Type: `null` | `Transaction`

Errors: `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransaction", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "type":"assetMint",
    "metadata":"...",
    "output":{
      "lockScriptHash":"0xf42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3",
      "parameters":[],
      "amount":10000
    },
    "registrar":null,
    "nonce":0
  },
  "id":null
}
```

## chain_getTransactionInvoice
Gets a transaction invoice with the given hash.

Params:
 1. transaction hash - `H256`

Return Type: `null` | "Success" | "Failed"

Errors: `Invalid Params`

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

## chain_getAssetSchemeByHash
Gets an asset scheme with the given asset type.

Params:
 1. transaction hash of AssetMintTransaction - `H256`
 2. shard id - `number`
 3. world_id - `number`

Return Type: `null` | `AssetScheme`

Errors: `KVDB Error`, `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetSchemeByHash", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0, 0], "id": null}' \
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

## chain_getAssetSchemeByType
Gets an asset scheme with the given asset type.

Params:
 1. asset type - `H256`

Return Type: `null` | `AssetScheme`

Errors: `KVDB Error`, `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetSchemeByType", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
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
 3. block number: `number` | `null`

Return Type: `null` | `Asset`

Errors: `KVDB Error`, `Invalid Params`

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

## chain_isAssetSpent
Checks whether an asset is spent or not.

Params:
 1. transaction hash: `H256`
 2. index: `number`
 3. shard id: `number`
 4. block number: `number` | `null`

Return Type: `null` | `false` | `true` - It returns null when no such asset exists.

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_isAssetSpent", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0, 0], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":false,
  "id":null
}
```

## chain_getNonce
Gets a nonce of an account of the given address, at state of the given blockNumber.

Params:
 1. address: `H160`
 2. block number: `number` | `null`

Return Type: `U256`

Errors: `KVDB Error`, `Invalid Params`

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

Errors: `KVDB Error`, `Invalid Params`

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

Errors: `KVDB Error`, `Invalid Params`

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

Errors: `KVDB Error`, `Invalid Params`

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

Errors: `KVDB Error`, `Invalid Params`

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

Return Type: `Parcel[]`

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
            "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7",
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
Gets coinbase's account id.

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

## chain_executeTransactions
Executes the transactions and returns the current shard root and the changed shard root.

Params:
 1. transactions: `Transaction[]`
 2. sender: `H160`

Return Type: `ChangeShard[]`

Errors: `Invalid RLP`, `Execution Failed`, `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_executeTransactions", "params": [[{"type":"assetMint","data":{"networkId":"17","shardId":0,"worldId":0,"metadata":"{\"name\":\"Gold\",\"description\":\"An asset example\",\"icon_url\":\"https://gold.image/\"}","output":{"lockScriptHash":"0xf42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3","parameters":[],"amount":10000},"registrar":null,"nonce":0}}, {"type":"assetMint","data":{"networkId":"17","shardId":1,"worldId":0,"metadata":"{\"name\":\"Gold\",\"description\":\"An asset example\",\"icon_url\":\"https://gold.image/\"}","output":{"lockScriptHash":"0xf42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3","parameters":[],"amount":10000},"registrar":null,"nonce":0}}], "0xa6594b7196808d161b6fb137e781abbc251385d9"], "id": null}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc": "2.0",
  "result": [
    {
      "postRoot": "0x16f176868ec7c8366af7e1210a98887437e1940c220d36e1264cec381bd8eae2",
      "preRoot": "0x3521429ad738442ad7aee37324331e5395bbd0aac7465fba8df12985f6fc2e60",
      "shardId": 0
    }, {
      "postRoot": "0x1d46e3dc3224ac963599c5350dd818b73f6b01efbeb3e19b7450b553d7c67cef",
      "preRoot": "0x1c41fc1cc2382352ab1a3dd45af8df70d1f2e8c77fc60f6c8849101d20ee7b3f",
      "shardId": 1
    }
  ],
  "id": null
}
```

## chain_getNetworkId
Return the nework id that is used in this chain.

Params: No parameters

Return Type: `number`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNetworkId", "params": [], "id": 6}' \
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

## miner_getWork
Returns the hash of the current block and score.

Params: No parameters

Return Type: `Work`

Errors: `No Author`, `No Work Required`, `No Work Found`

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

Errors: `No Work Required`, `Invalid Params`

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

Errors: `Invalid Params`

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

Errors: `Invalid Params`

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

Errors: `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_isConnected", "params": ["192.168.0.3", 3485], "id": 6}' \
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

Return Type: null

Errors: `Not Conntected`, `Invalid Params`

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
  "result":null,
  "id":6
}
```

## net_getPeerCount
Return the count of peers which the client is connected to.

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
Return the port number on which the client is listening for peers.

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

## account_getList
Gets a list of accounts.

Params: No parameters

Return Type: `PlatformAddress[]`

Errors: `Keystore Error`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_getList", "params": [], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":["0x318def87d8dc0f7cc21794daf2dd36762db22b67"],
  "id":6
}
```

## account_create
Creates a new account.

Params:
 1. password: `string` | `null`

Return Type: `PlatformAddress`

Errors: `Keystore Error`, `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_create", "params": [], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x318def87d8dc0f7cc21794daf2dd36762db22b67",
  "id":6
}
```

## account_importRaw
Imports a secret key and add the corresponding account.

Params:
 1. secret: `H256`
 2. password: `string` | `null`

Return Type: `PlatformAddress`

Errors: `Keystore Error`, `Key Error`, `Already Exists`, `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_importRaw", "params": ["a2b39d4aefecdb17f84ed4cf629e7c8817691cc4f444ac7522902b8fb4b7bd53"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xa22ae626d26923bdd9321e648de080c18e1049f2",
  "id":6
}
```

## account_remove
Removes the account

Params:
 1. account: `PlatformAddress`
 2. password: `string` | `null`

Return type: `null`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_remove", "params": ["1228c0de48fdc303b4b7f51049ae2887358f94b6"], "id": 6}' \
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":6
}
```

## account_sign
Calculates the account's signature for a given message.

Params:
 1. message: `H256`
 2. account: `PlatformAddress`
 3. password: `string` | `null`

Return type: `Signature`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_sign", "params": ["0000000000000000000000000000000000000000000000000000000000000000", "1228c0de48fdc303b4b7f51049ae2887358f94b6"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xff7e8928f7758a64b9ea6c53f9945cdd223740675ac6ac6da625306d3966f8197523e00d56844ddb70631d44f045f4d83cc183a267c3182ab04c2f459c8289f501",
  "id":6
}
```

## account_changePassword
Changes the account's password

Params:
 1. account: `PlatformAddress`
 2. old_password: `String`
 3. new_password: `String`

Return Type: `null`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_changePassword", "params": ["0x318def87d8dc0f7cc21794daf2dd36762db22b67", "1234", "5678"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":6
}
```

## shardValidator_registerAction
Sends an action to get signatures. The action will be propagated and shard
validators will send the signatures of the action if it is a valid action.

Params:
 1. action: `Action`

Return Type: `bool`

Errors: `Invalid Params`

Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "shardValidator_registerAction", "params": [{"action":"changeShardState","transactions":[{"type":"assetMint","data":{"networkId":17,"shardId":0,"metadata":"{\"name\":\"Gold\",\"description\":\"An asset example\",\"icon_url\":\"https://gold.image/\"}","output":{"lockScriptHash":"0xf42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3","parameters":[],"amount":10000},"registrar":null,"nonce":0}}],"changes":[{"shardId":0,"preRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","postRoot":"0x0000000000000000000000000000000000000000000000000000000000000000"}]}
], "id": null}' \
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

## shardValidator_getSignatures
Gets the signatures signed by the shard validators for the given action.

Params:
 1. action_hash: `H256`

Return type: `Signature[]`

Errors: `Invalid Params`

Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "shardValidator_getSignatures", "params": ["0xa2b39d16efe74b17f84ed4cf629e7c8817691cc4f444ac7522902b8fb4b7bd53"], "id": 6}' \
    localhost:8080
```

Response Example
```
{
  "jsonrpc":"2.0",
  "result":["0xff7e8928f7758a64b9ea6c53f9945cdd223740675ac6ac6da625306d3966f8197523e00d56844ddb70631d44f045f4d83cc183a267c3182ab04c2f459c8289f501"],
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
