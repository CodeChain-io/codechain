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

## U64, U128, U256, ...

A hexadecimal string for XXX-bit unsigned integer

## NetworkID

A two-letter string to denote a network. For example, "cc" is for the main network, and "tc" is for the Husky test network. See [the specification](List-of-Network-Id.md).

## PlatformAddress

A string that starts with "(NetworkID)c", and Bech32 string follows. For example, "cccqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqz6sxn0" is for the main network, and "tccqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqz6sxn0" is for the Husky test network. See [the specification](CodeChain-Address.md#1-platform-account-address-format).

## Block

 - author: `PlatformAddress`
 - extraData: `any[]`
 - hash: `H256`
 - invoicesRoot: `H256`
 - number: `number`
 - transactions: `Transaction[]`
 - transactionsRoot: `H256`
 - parentHash: `H256`
 - score: `number`
 - seal: `string[]`
 - stateRoot: `H256`
 - timestamp: `number`

## Transaction

 - blockHash: `H256`
 - blockNumber: `number`
 - fee: `U64`
 - hash: `H256`
 - networkId: `NetworkID`
 - seq: `number`
 - transactionIndex: `number`
 - sig: `Signature`
 - action: `Action`

## UnsignedTransaction

 - fee: `U64`
 - networkId: `NetworkID`
 - seq: `number` | `null`
 - action: `Action`

## Actions

### MintAsset Action

 - networkId: `NetworkID`
 - shardId: `number`
 - metadata: `string`
 - output: `AssetMintOutput`
 - approver: `PlatformAddress` | `null`
 - approvals: `Signature[]`

### TranferAsset Action

 - networkId: `NetworkID`
 - burns: `AssetTransferInput[]`
 - inputs: `AssetTransferInput[]`
 - outputs: `AssetTransferOutput[]`
 - orders: `OrderOnTransfer[]`
 - approvals: `Signature[]`

### ComposeAsset Action

 - networkId: `NetworkID`
 - shardId: `number`
 - metadata: `string`
 - inputs: `AssetTransferInput[]`
 - output: `AssetMintOutput`
 - approver: `PlatformAddress` | `null`
 - approvals: `Signature[]`

### DecomposeAsset Action

 - networkId: `NetworkID`
 - input: `AssetTransferInput`
 - outputs: `AssetTransferOutput[]`
 - approvals: `Signature[]`

### UnwrapCCC Action

 - networkId: `NetworkID`
 - burn: `AssetTransferInput`

### Pay Action

 - type: "pay"
 - receiver: `PlatformAddress`
 - amount: `U64`

### SetRegularKey Action

 - type: "setRegularKey"
 - key: `H512`

### SetShardOwners Action

 - type: "setShardOwners"
 - shardId: `number`
 - owners: `PlatformAddress[]`

### SetShardUsers Action

 - type: "setShardUsers"
 - shardId: `number`
 - users: `PlatformAddress[]`

### WrapCCC Action

 - type: "wrapCCC"
 - shardId: `number`
 - lockScriptHash: `H160`
 - parameters: `number[][]`
 - amount: `U64`

### Store Action

 - type: "store"
 - content: `string`
 - certifier: `PlatformAddress`
 - signature: `Signature`

### Remove Action

 - type: "remove"
 - hash: `H256` - transaction hash
 - signature: `Signature`

### Custom Action

 - type: "custom"
 - handlerId: `number`
 - bytes: `string`

## AssetScheme

 - amount: `U64`
 - metadata: `string`
 - approver: `PlatformAddress` | `null`

## Asset

 - amount: `U64`
 - assetType: `H256`
 - lockScriptHash: `H160`
 - parameters: `number[][]`

## Text

 - content: `string`
 - certifier: `PlatformAddress`

## Transactions

 - type: "assetMint" | "assetTransfer" | "assetCompose" | "assetDecompose" | "assetUnwrapCCC"
 - data: `AssetMintData` | `AssetTransferData` | `AssetComposeData` | `AssetDecomposeData` | `AssetUnwrapCCCData`

### Transaction in Response

When `Transaction` is included in any response, there will be an additional field `hash` in the data, which is the hash value of the given transaction. This decreases the time to calculate the transaction hash when it is needed from the response.

### AssetMintOutput

 - lockScriptHash: `H160`
 - parameters: `number[][]`
 - amount: `U64` | `null`

### AssetTransferInput

 - prevOut: `AssetOutPoint`
 - timelock: `Timelock`
 - lockScript: `number[]`
 - unlockScript: `number[]`

#### Timelock

 - type: "block" | "blockAge" | "time" | "timeAge"
 - value: `number`

#### AssetOutPoint

 - transactionId: `H256`
 - index: `number`
 - assetType: `H256`
 - amount: `U64`

### AssetTransferOutput

 - lockScriptHash: `H160`
 - parameters: `number[][]`
 - assetType: `H256`
 - amount: `U64`

### Order

 - assetTypeFrom: `H256`
 - assetTypeTo: `H256`
 - assetTypeFee: `H256`
 - assetAmountFrom: `U64`
 - assetAmountTo: `U64`
 - assetAmountFee: `U64`
 - originOutputs: `AssetOutPoint[]`
 - expiration: `number`
 - lockScriptHash: `H160`
 - parameters: `number[][]`

### OrderOnTransfer

 - order: `Order`
 - spentAmount: `U64`
 - inputIndices: `number[]`
 - outputIndices: `number[]`

## Signature
`H520` for ECDSA signature | `H512` for Schnorr signature

# Error codes

|  Code  |         Message        |                          Description                         |
|--------|------------------------|--------------------------------------------------------------|
| -32002 | `No Author`            | No author is configured                                      |
| -32004 | `No Work Required`     | No work is required                                          |
| -32005 | `No Work Found`        | No work is found                                             |
| -32009 | `Invalid RLP`          | Failed to decode the RLP string                              |
| -32011 | `KVDB Error`           | Failed to access the state (Internal error of CodeChain)     |
| -32010 | `Execution Failed`     | Failed to execute the transactions                           |
| -32030 | `Verification Failed`  | The signature is invalid                                     |
| -32031 | `Already Imported`     | The same transaction is already imported                     |
| -32032 | `Not Enough Balance`   | The signer's balance is insufficient                         |
| -32033 | `Too Low Fee`          | The fee is lower than the minimum required                   |
| -32034 | `Too Cheap to Replace` | The fee is lower than the existing one in the queue          |
| -32035 | `Invalid Seq`          | The signer's seq is invalid to import                        |
| -32036 | `Invalid NetworkId`    | The network id does not match                                |
| -32040 | `Keystore Error`       | Failed to access the key store (Internal error of CodeChain) |
| -32041 | `Key Error`            | The key is invalid                                           |
| -32042 | `Already Exists`       | The account already exists                                   |
| -32043 | `Wrong Password`       | The password does not match                                  |
| -32044 | `No Such Account`      | There is no such account in the key store                    |
| -32045 | `Not Unlocked`         | The account is not unlocked                                  |
| -32046 | `Transfer Only`        | chain_executeVM() only accepts AssetTransfer transactions    |
| -32099 | `Unknown Error`        | An unknown error occurred                                    |
| -32602 | `Invalid Params`       | At least one of the parameters is invalid                    |

# List of methods

 * [ping](#ping)
 * [version](#version)
 * [commitHash](#commithash)
***
 * [chain_getBestBlockNumber](#chain_getbestblocknumber)
 * [chain_getBestBlockId](#chain_getbestblockid)
 * [chain_getBlockHash](#chain_getblockhash)
 * [chain_getBlockByNumber](#chain_getblockbynumber)
 * [chain_getBlockByHash](#chain_getblockbyhash)
 * [chain_sendSignedTransaction](#chain_sendsignedtransaction)
 * [chain_getTransaction](#chain_gettransaction)
 * [chain_getInvoice](#chain_getinvoice)
 * [chain_getTransactionById](#chain_gettransactionbyid)
 * [chain_getInvoicesById](#chain_getinvoicesbyid)
 * [chain_getAssetSchemeByHash](#chain_getassetschemebyhash)
 * [chain_getAssetSchemeByType](#chain_getassetschemebytype)
 * [chain_getAsset](#chain_getasset)
 * [chain_getText](#chain_gettext)
 * [chain_isAssetSpent](#chain_isassetspent)
 * [chain_getSeq](#chain_getseq)
 * [chain_getBalance](#chain_getbalance)
 * [chain_getRegularKey](#chain_getregularkey)
 * [chain_getRegularKeyOwner](#chain_getregularkeyowner)
 * [chain_getGenesisAccounts](#chain_getgenesisaccounts)
 * [chain_getNumberOfShards](#chain_getnumberofshards)
 * [chain_getShardRoot](#chain_getshardroot)
 * [chain_getPendingTransactions](#chain_getpendingtransactions)
 * [chain_getMiningReward](#chain_getminingreward)
 * [chain_executeTransaction](#chain_executetransaction)
 * [chain_executeVM](#chain_executevm)
 * [chain_getNetworkId](#chain_getnetworkid)
***
 * [engine_getCoinbase](#engine_getcoinbase)
 * [engine_getBlockReward](#engine_getblockreward)
 * [engine_getRecommendedConfimation](#engine_getrecommendedconfimation)
***
 * [miner_getWork](#miner_getwork)
 * [miner_submitWork](#miner_submitwork)
***
 * [net_shareSecret](#net_sharesecret)
 * [net_connect](#net_connect)
 * [net_isConnected](#net_isconnected)
 * [net_disconnect](#net_disconnect)
 * [net_getPeerCount](#net_getpeercount)
 * [net_getEstablishedPeers](#net_getestablishedpeers)
 * [net_getPort](#net_getport)
 * [net_addToWhitelist](#net_addtowhitelist)
 * [net_removeFromWhitelist](#net_removefromwhitelist)
 * [net_addToBlacklist](#net_addtoblacklist)
 * [net_removeFromBlacklist](#net_removefromblacklist)
 * [net_enableWhitelist](#net_enablewhitelist)
 * [net_disableWhitelist](#net_disablewhitelist)
 * [net_enableBlacklist](#net_enableblacklist)
 * [net_disableBlacklist](#net_disableblacklist)
 * [net_getWhitelist](#net_getwhitelist)
 * [net_getBlacklist](#net_getblacklist)
***
 * [account_getList](#account_getlist)
 * [account_create](#account_create)
 * [account_importRaw](#account_importraw)
 * [account_unlock](#account_unlock)
 * [account_sign](#account_sign)
 * [account_sendTransaction](#account_sendtransaction)
 * [account_changePassword](#account_changepassword)
***
 * [devel_getStateTrieKeys](#devel_getstatetriekeys)
 * [devel_getStateTrieValue](#devel_getstatetrievalue)
 * [devel_startSealing](#devel_startsealing)
 * [devel_stopSealing](#devel_stopsealing)
 * [devel_getBlockSyncPeers](#devel_getblocksyncpeers)


# Specification

## ping
Sends ping to check whether CodeChain's RPC server is responding or not.

### Params
No parameters

### Returns
`string` - "pong"

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "ping", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"pong",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## version
Gets the version of CodeChain.

### Params
No parameters

### Returns
`string` - e.g. 0.1.0

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "version", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0.1.0",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## commitHash
Gets the commit hash of the repository upon which the CodeChain executable was built.

### Params
No parameters

### Returns
`string` - the commit hash

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "commitHash", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": "361a36fe20900f15e71148a615b25978652bfe90",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBestBlockNumber
Gets the number of the best block.

### Params
No parameters

### Returns
`number`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBestBlockNumber", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":1,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBestBlockId
Gets the number and the hash of the best block.

### Params
No parameters

### Returns
{ hash: `H256`, number: `number` }

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBestBlockId", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "hash":"0x7f7104b580f9418d444560009e5a92a4573d42d2c51cd0c6045afdc761826249",
    "number":1
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBlockHash
Gets the hash of the block with given number.

### Params
 1. n - `number`

### Returns
`null` | `H256`

Errors: `Invalid Params`

### Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockHash", "params": [1], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBlockByNumber
Gets the block with the given number.

### Params
 1. number: `number`

### Returns
`null` | `Block`

Errors: `Invalid Params`

### Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockByNumber", "params": [5], "id": null}' \
    http://localhost:8080
```

### Response Example:
```
{
  "jsonrpc":"2.0",
  "result":{
    "author":"sccqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqz6sxn0",
    "extraData":[

    ],
    "hash":"0x0e9cbbe0ecc774de3b5d05827ffb5c541bc7b7ff63de253d17272cf0fea1b7af",
    "invoicesRoot":"0x6db236c944eda064237e88be9cddf7766ce877fe0c4414ac5999f4f5429750fd",
    "number":5,
    "transactions":[
      {
        "action":{
          "type":"pay",
          "amount":"0x3b9aca00",
          "receiver":"sccqra5felweesff3epv9wfu05a47sxh89yuvzw7mqd"
        },
        "blockHash":"0x0e9cbbe0ecc774de3b5d05827ffb5c541bc7b7ff63de253d17272cf0fea1b7af",
        "blockNumber":5,
        "fee":"0x5f5e100",
        "hash":"0x3ff9b02427ac04c06260928168775bca5a3da96ae6995041e197d42e71ab68b6",
        "networkId":"sc",
        "seq": 4,
        "transactionIndex":0,
        "sig":"0x4621da0344d8888c5076cc0a3cc7fd7a7e3a761ba812c95f807c050a4e5ec6b7120fa99fdf502ed088ed61eb6d5fe44f44c280e97c7702d5127640d7a8a6d7e401"
      }
    ],
    "transactionsRoot":"0xa4a8229a90d91e9a38b17f95c9ac2d01f46b10553e62c68df5bbfe1cc5b3e164",
    "parentHash":"0xbc4f7e7b1dded863c500147243d78436ca297bfae64e1ec2d17396286cf14b6e",
    "score":"0x20000",
    "seal":[

    ],
    "stateRoot":"0x4cdbde0340558aa7116975a170f004af3b6343f5bf0354dadd1815d22ed12da7",
    "timestamp":1536924583
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBlockByHash
Gets the block with the given hash.

### Params
 1. hash: `H256`

### Returns
`null` | `Block`

Errors: `Invalid Params`

### Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBlockByHash", "params": ["0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50"], "id": null}' \
    localhost:8080
```

### Response Example
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
    "transactions":[
      {
        "action":{
          "type":"pay",
          "amount":"0xa",
          "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"
        },
        "blockHash":"0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
        "blockNumber":5,
        "fee":"0xa",
        "hash":"0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
        "networkId":"cc",
        "seq": 4,
        "transactionIndex":0,
        "sig":"0x291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"
      }
    ],
    "transactionsRoot":"0x0270d11d2bd21a0ec8e78d1c4e918103d7c4b02fdf734051231cb9eea90ae88e",
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

[Back to **List of methods**](#list-of-methods)

## chain_sendSignedTransaction
Sends a signed transaction, returning its hash.

### Params
 1. bytes: `hexadecimal string` - RLP encoded hex string of SignedTransaction

### Returns
`H256` - transaction hash

Errors: `Invalid RLP`, `Verification Failed`, `Already Imported`, `Not Enough Balance`, `Too Low Fee`, `Too Cheap to Replace`, `Invalid Seq`, `Invalid Params`, `Invalid NetworkId`

### Request Example:
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_sendSignedTransaction", "params": ["0xf85e040a11d70294a6594b7196808d161b6fb137e781abbc251385d90ab841291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getTransaction
Gets a transaction with the given hash.

### Params
 1. transaction hash - `H256`

### Returns
`null` or `Transaction`

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransaction", "params": ["0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
    "jsonrpc": "2.0",
    "result": {
        "action": {
          "type":"pay",
          "amount":"0xa",
          "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"
        },
        "blockHash": "0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
        "blockNumber": 5,
        "fee": "0xa",
        "hash": "0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
        "networkId": "cc",
        "seq": 4,
        "transactionIndex": 0,
        "sig":"0x291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"
    }
    "id": null,
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getInvoice
Gets a transaction invoice with the given hash.

### Params
 1. transaction hash - `H256`

### Returns
`null` | `string[]` - Each string is either "Success" or "Failed"

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getInvoice", "params": ["0xad708d48755ac36685280a45ec213941e21c41644c781bf2f487fd6c7e4b2ebb"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "Success"
  ],
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getTransactionById
Gets a transaction with the given transaction id.

### Params
 1. transaction id - `H256`

### Returns
`null` | `Transaction`

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransactionById", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
    "jsonrpc": "2.0",
    "result": {
        "action": {
          "type":"pay",
          "amount":"0xa",
          "receiver": "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"
          "hash": "0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc",
        },
        "blockHash": "0xfc196ede542b03b55aee9f106004e7e3d7ea6a9600692e964b4735a260356b50",
        "blockNumber": 5,
        "fee": "0xa",
        "hash": "0xdb7c705d02e8961880783b4cb3dc051c41e551ade244bed5521901d8de190fc6",
        "networkId": "cc",
        "seq": 4,
        "transactionIndex": 0,
        "sig":"0x291d932e55162407eb01915923d68cf78df4815a25fc6033488b644bda44b02251123feac3a3c56a399a2b32331599fd50b7a39ec2c1a2325e37f383c6aeedc301"
    }
    "id": null,
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getInvoicesById
Gets transaction invoices with the given transaction id.

### Params
 1. transaction id - `H256`

### Returns
`string[]` - Each string is either "Success" or "Failed".

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getInvoicesById", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": ["Failed", "Success"],
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getAssetSchemeByHash
Gets an asset scheme with the given asset type.

### Params
 1. transaction id of AssetMintTransaction - `H256`
 2. shard id - `number`
 3. block number: `number` | `null`

### Returns
`null` | `AssetScheme`

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetSchemeByHash", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0, null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "amount":100,
    "metadata":"",
    "approver":null
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getAssetSchemeByType
Gets an asset scheme with the given asset type.

### Params
 1. asset type - `H256`
 2. block number: `number` | `null`

### Returns
`null` | `AssetScheme`

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAssetSchemeByType", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "amount":100,
    "metadata":"",
    "approver":null
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getAsset
Gets an asset with the given asset type.

### Params
 1. transaction id - `H256`
 2. index - `number`
 3. block number: `number` | `null`

### Returns
`null` | `Asset`

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getAsset", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "amount":100,
    "assetType":"0x53000000000000002ec1193ecd52e2833ffc10b45bea1fda49f857e34db67c68",
    "lockScriptHash":"0x0000000000000000000000000000000000000000",
    "parameters":[

    ]
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getText
Gets the text with given transaction hash.

### Params
 1. transaction hash - `H256` - Hash of signed transaction
 2. block number: `number` | `null`

### Returns
`null` | `Text`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getText", "params": ["0xd04303364ed7658fa2fba39a72ef5f0bb1308a23b42fd565f5949fc9b68485e5", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":{
    "content": "CodeChain",
    "certifier": "tccqy6r92677phvflf0g08wgevum33jsavvmcl53d7e",
  },
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_isAssetSpent
Checks whether an asset is spent or not.

### Params
 1. transaction id: `H256`
 2. index: `number`
 3. shard id: `number`
 4. block number: `number` | `null`

### Returns
`null` | `false` | `true` - It returns null when no such asset exists.

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_isAssetSpent", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", 0, 0], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":false,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getSeq
Gets a seq of an account of the given address, at state of the given blockNumber.

### Params
 1. address: `PlatformAddress`
 2. block number: `number` | `null`

### Returns
`null` | `number` - It returns null when the given block number is invalid.

Errors: `KVDB Error`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getSeq", "params": ["cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": 84,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getBalance
Gets a balance of an account of the given address, at the state of the given blockNumber.

### Params
 1. address: `PlatformAddress`
 2. block number: `number` | `null`

### Returns
`null` | `U64` - It returns null when the given block number is invalid.

Errors: `KVDB Error`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getBalance", "params": ["cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xe8d4a50dd0",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getRegularKey
Gets the regular key of an account of the given address, at the state of the given blockNumber.

### Params
 1. address: `PlatformAddress`
 2. block number: `number` | `null`

### Returns
`null` | `H512` - 512-bit public key. It returns null when the given address does not have a regular key.

Errors: `KVDB Error`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getRegularKey", "params": ["cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getRegularKeyOwner
Gets the owner of a regular key, at the state of the given blockNumber.

### Params
 1. public key: `H512`
 2. block number: `number` | `null`

### Returns
`null` | `PlatformAddress` - It returns null when the given key has no owner.

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getRegularKeyOwner", "params": ["0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getGenesisAccounts
Gets the platform account in the genesis block.

### Params
No parameters

### Returns
`PlatformAddress[]` - It returns the array of the platform address

Errors: `KVDB Error`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getGenesisAccounts", "params": [], "id": 37}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": ["cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"],
  "id":37
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getNumberOfShards
Gets the number of shards, at the state of the given blockNumber.

### Params
 1. block number: `number` | `null`

### Returns
`number` - the number of shards

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNumberOfShards", "params": [null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":3,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getShardRoot
Gets the root of shard, at the state of the given blockNumber.

### Params
 1. shard id: `number`
 2. block number: `number` | `null`

### Returns
`null` | `H256` - the root of shard

Errors: `KVDB Error`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getShardRoot", "params": [1, null], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xf3841adc1615bfeabb801dda23585c1722b80d810df084a5f2198e92285d4bfd",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getPendingTransactions
Gets transactions in the current transaction queue.

### Params
No parameters

### Returns
`Transaction[]`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getPendingTransactions", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    {
      "blockHash":null,
      "blockNumber":null,
      "fee":"0xa",
      "hash":"0x8ae3363ccdcc02d8d662d384deee34fb89d1202124e8065f0d6c84ab31e68d8a",
      "networkId":"cc",
      "seq":"0x0",
      "transactionIndex":null,
      "r":"0x22605d6b9fb713d3a415e02eeed8b4a630e0d867c91bf7d9b7721f94159c0fe1",
      "s":"0x772f19f1c27f1db8b28289caa9e99ad756878fd56b2415c25cd47cc737f7e0c2",
      "transactions":[
        {
          "pay":{
            "seq": 1,
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

[Back to **List of methods**](#list-of-methods)

## chain_getMiningReward
Gets the mining reward of the given block number.
Unlike `engine_getBlockReward`, it returns the actual amount received, including the transaction fee.
It returns `null` if the given block number is not mined yet.

### Params
 1. block number: `number`

### Returns
`U64` | `null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getMiningReward", "params": [10], "id": 41}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":41
}
```

[Back to **List of methods**](#list-of-methods)

## chain_executeTransaction
Executes the transactions and returns the current shard root and the changed shard root.

### Params
 1. transaction: `Transaction`
 2. sender: `PlatformAddress`

### Returns
`Invoice`

Errors: `Invalid RLP`, `Execution Failed`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_executeTransaction", "params": [{"type":"assetMint","data":{"networkId":"cc","shardId":0,"metadata":"{\"name\":\"Gold\",\"description\":\"An asset example\",\"icon_url\":\"https://gold.image/\"}","output":{"lockScriptHash":"0xf42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3","parameters":[],"amount":10000},"approver":null,"nonce":0}}, "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7"], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "Success"
  ],
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_executeVM
Execute the inputs of the AssetTransfer transaction in the CodeChain VM, and return the results. This does not run the VM on burns.

### Params
 1. transaction: `Transaction`
 2. parameters: `number[][][]` - Provide parameters of outputs as an array.
 3. indices: `number[]` - Provide indices of inputs to run in VM.

* The length of `parameters` and `indices` must be equal.

### Returns
`("unlocked"|"burnt"|"failed"|"invalid")[]`

Errors: `Transfer Only`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_executeVM", "params": [{"type":"assetTransfer","data":{"networkId":"tc","burns":[],"inputs":[{"prevOut":{"transactionHash":"0x56774a7e53abd17d70789af6d6f89b4ac23048c07430d1fbe7a8fe0688ecd250","index":0,"assetType":"0x53000000ec7f404207fc5f6bfaad91ed3bf4532b94f508fbea86223409eb189c","amount":"0x64"},"timelock":null,"lockScript":[53,1,148,17,34,255,128],"unlockScript":[50,65,57,113,98,163,242,125,128,229,140,240,213,154,218,70,232,138,150,84,215,67,109,128,156,81,100,57,53,194,83,70,149,63,53,138,140,11,7,42,34,206,32,244,60,3,191,57,24,132,44,10,175,13,218,20,62,152,175,40,8,240,76,185,246,37,0,50,1,3,50,64,179,217,97,169,96,174,90,169,141,98,170,45,70,139,251,168,8,238,200,83,24,49,115,158,81,199,69,29,229,191,88,173,232,249,178,39,56,223,68,148,75,92,15,236,37,56,88,197,38,111,93,69,232,65,2,247,239,134,191,115,159,238,196,201]}],"outputs":[{"lockScriptHash":"0x5f5960a7bca6ceeeb0c97bc717562914e7a1de04","parameters":[[170,45,255,58,152,57,253,189,84,170,233,14,217,172,65,78,188,106,99,109]],"assetType":"0x53000000ec7f404207fc5f6bfaad91ed3bf4532b94f508fbea86223409eb189c","amount":"0x64"}],"orders":[]}}, [[[123,110,101,117,85,125,64,83,80,25,37,104,84,81,160,50,198,212,89,125]]], [0]], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "unlocked"
  ],
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## chain_getNetworkId
Return the nework id that is used in this chain.

### Params
No parameters

### Returns
`number`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getNetworkId", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": 17,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## engine_getCoinbase
Gets coinbase's account id.

### Params
No parameters

### Returns
`PlatformAddress` | `null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "engine_getCoinbase", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7",
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## engine_getBlockReward
Gets the reward of the given block number

### Params
 1. block number: `number`

### Returns
`U64`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "engine_getBlockReward", "params": [10], "id": 41}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0x50",
  "id":41
}
```

[Back to **List of methods**](#list-of-methods)

## engine_getRecommendedConfirmation
Gets the recommended minimum confirmations.

### Params
No parameters

### Returns
`number`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "engine_getRecommendedConfirmation", "params": [], "id": 411}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": 6,
  "id":411
}
```

[Back to **List of methods**](#list-of-methods)

## engine_getCustomActionData
Gets custom action data for given custom action handler id and rlp encoded key.

### Params
 1. handlerId: `number`
 2. bytes: `string`
 3. blockNumber: `number` | `null`

### Returns
`string`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "engine_getCustomActionData", "params": [1,"0xcd8c6d6574616461746120686974",null], "id": 411}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0c",
  "id":411
}
```

[Back to **List of methods**](#list-of-methods)

## miner_getWork
Returns the hash of the current block and score.

### Params
No parameters

### Returns
`Work`

Errors: `No Author`, `No Work Required`, `No Work Found`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_getWork", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
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

[Back to **List of methods**](#list-of-methods)

## miner_submitWork
Used for submitting a proof-of-work solution.

### Params
 1. powHash: `string`
 2. seal: `string[]`

### Returns
`bool`

Errors: `No Work Required`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "miner_submitWork", "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"]], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":true,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_shareSecret
Share secret to the given address.

### Params
 1. secret: `string`
 2. address: `string`
 3. port: `number`

### Returns
`null`

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_shareSecret", "params": ["0x24df02abcd4e984e90253dc344e89b8431bbb319c66643bfef566dfdf46ec6bc", "192.168.0.3", 3485], "id": 5}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":5
}
```

[Back to **List of methods**](#list-of-methods)

## net_connect
Connect to the given address.

### Params
 1. address: `string`
 2. port: `number`

### Returns
`null`

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_connect", "params": ["192.168.0.3", 3485], "id": 5}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":5
}
```

[Back to **List of methods**](#list-of-methods)

## net_isConnected
Check whether the connection is established.

### Params
 1. address: `string`
 2. port: `number`

### Returns
`bool`

Errors: `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_isConnected", "params": ["192.168.0.3", 3485], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":true,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_disconnect
Disconnect the connection from the given address.

### Params
 1. address: `string`
 2. port: `number`

### Returns
`null`

Errors: `Not Conntected`, `Invalid Params`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_disconnect", "params": ["192.168.0.3", 3485], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_getPeerCount
Return the count of peers which the client is connected to.

### Params
No parameters

### Returns
`number`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getPeerCount", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": 34,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_getEstablishedPeers
Return the socket addresses of established peers.

### Params
No parameters

### Returns
`string[]`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getEstablishedPeers", "params": [], "id": 3}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": ["1.2.3.4:3485", "1.2.3.5:3485"],
  "id":3
}
```

[Back to **List of methods**](#list-of-methods)

## net_getPort
Return the port number on which the client is listening for peers.

### Params
No parameters

### Returns
`number`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getPort", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": 3485,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_addToWhitelist
Adds the address to the whitelist.

### Params
 1. address: `string`
 2. tag: `null` | `string`

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_addToWhitelist", "params": ["1.2.3.4", "tag"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_removeFromWhitelist
Removes the address from the whitelist.

### Params
 1. address: `string`

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_removeFromWhitelist", "params": ["1.2.3.4"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_addToBlacklist
Adds the address to the blacklist.

### Params
 1. address: `string`
 2. tag: `null` | `string`

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_addToBlacklist", "params": ["1.2.3.4", "tag"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_removeFromBlacklist
Removes the address from the blacklist.

### Params
 1. address: `string`

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_removeFromBlacklist", "params": ["1.2.3.4"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_enableWhitelist
Enables whitelist.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_enableWhitelist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_disableWhitelist
Disables whitelist.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_disableWhitelist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_enableBlacklist
Enables blacklist.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_enableBlacklist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_disableBlacklist
Disables blacklist.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_disableBlacklist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_getWhitelist
Gets the address in the whitelist.

### Params
No parameters

### Returns
{ list: `string[][]`, enabled: `bool` }

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getWhitelist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": { "list": [["1.2.3.4", "tag1"], ["1.2.3.5", "tag2"], ["1.2.3.6", "tag3"]], "enabled": true },
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## net_getBlacklist
Gets the address in the blacklist.

### Params
No parameters

### Returns
{ list: `string[][]`, enabled: `bool` }

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getBlacklist", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": { "list": [["1.2.3.4", "tag1"], ["1.2.3.5", "tag2"], ["1.2.3.6", "tag3"]], "enabled": false },
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_getList
Gets a list of accounts.

### Params
No parameters

### Returns
`PlatformAddress[]`

Errors: `Keystore Error`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_getList", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":["cccqqccmmu8mrwq7lxzz72d4ukaxemzmv3tvues8uwy"],
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_create
Creates a new account.

### Params
 1. password: `string` | `null`

### Returns
`PlatformAddress`

Errors: `Keystore Error`, `Invalid Params`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_create", "params": [], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"cccqqccmmu8mrwq7lxzz72d4ukaxemzmv3tvues8uwy",
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_importRaw
Imports a secret key and add the corresponding account.

### Params
 1. secret: `H256`
 2. password: `string` | `null`

### Returns
`PlatformAddress`

Errors: `Keystore Error`, `Key Error`, `Already Exists`, `Invalid Params`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_importRaw", "params": ["a2b39d4aefecdb17f84ed4cf629e7c8817691cc4f444ac7522902b8fb4b7bd53"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"cccqz3z4e3x6f5j80wexg0xfr0qsrqcuyzf7g4y0je6",
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_unlock
Unlocks the specified account for use.

It will default to 300 seconds. Passing 0 unlocks the account indefinitely.

### Params
 1. account: `PlatformAddress`
 2. password: `string`
 3. duration: `number`  | `null`

### Returns
`null`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_unlock", "params": ["cccqqccmmu8mrwq7lxzz72d4ukaxemzmv3tvues8uwy", "1234", 0], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result": null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_sign
Calculates the account's signature for a given message.

### Params
 1. message: `H256`
 2. account: `PlatformAddress`
 3. password: `string` | `null`

### Returns
`Signature`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Not Unlocked`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_sign", "params": ["0000000000000000000000000000000000000000000000000000000000000000", "cccqqfz3sx7fr7uxqa5kl63qjdw9zrntru5kcdsjywj"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":"0xff7e8928f7758a64b9ea6c53f9945cdd223740675ac6ac6da625306d3966f8197523e00d56844ddb70631d44f045f4d83cc183a267c3182ab04c2f459c8289f501",
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_sendTransaction
Sends a transaction by signing it with the account’s private key.
It automatically fills the seq if the seq is not given.

### Params
 1. transction: `UnsignedTransaction`
 2. account: `PlatformAddress`
 3. passphrase: `string` | `null`

### Returns
{ hash: `H256`, seq: `number` } - the hash and seq of the transaction

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Not Unlocked`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_sendTransaction", "params": [{"action":{ "type":"pay", "amount":"0x3b9aca00", "receiver":"sccqra5felweesff3epv9wfu05a47sxh89yuvzw7mqd" }, "fee":"0x5f5e100", "networkId":"sc", "seq": null}, "cccqqfz3sx7fr7uxqa5kl63qjdw9zrntru5kcdsjywj", null], "id": 6}' \
    localhost:8080
```


### Response Example
```
{
  "jsonrpc":"2.0",
  "result": {"seq": 999999999440, "hash":"0x8ae3363ccdcc02d8d662d384deee34fb89d1202124e8065f0d6c84ab31e68d8a"},
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## account_changePassword
Changes the account's password.

### Params
 1. account: `PlatformAddress`
 2. old_password: `String`
 3. new_password: `String`

### Returns
`null`

Errors: `Keystore Error`, `Wrong Password`, `No Such Account`, `Invalid Params`, `Invalid NetworkId`

### Request Example
```
curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "account_changePassword", "params": ["cccqqccmmu8mrwq7lxzz72d4ukaxemzmv3tvues8uwy", "1234", "5678"], "id": 6}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":6
}
```

[Back to **List of methods**](#list-of-methods)

## devel_getStateTrieKeys
Gets keys of the state trie with the given offset and limit.

### Params
 1. offset: `number`
 2. limit: `number`

### Returns
`H256[]` with maximum length _limit_

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieKeys", "params": [0, 1], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":[
    "0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"
  ],
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## devel_getStateTrieValue
Gets the value of the state trie with the given key.

### Params
 1. key: `string`

### Returns
`string[]` - each string is RLP encoded

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getStateTrieValue", "params": ["0x00acf5cba5c53e11f1512b8b480521cb546e7a17a96235a9282f6253b90de043"], "id": null}' \
    localhost:8080
```

### Response Example
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

[Back to **List of methods**](#list-of-methods)

## devel_startSealing
Starts and enables sealing blocks by the miner.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_startSealing", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## devel_stopSealing
Stops and disables sealing blocks by the miner.

### Params
No parameters

### Returns
`null`

### Request Example
```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_stopSealing", "params": [], "id": null}' \
    localhost:8080
```

### Response Example
```
{
  "jsonrpc":"2.0",
  "result":null,
  "id":null
}
```

[Back to **List of methods**](#list-of-methods)

## devel_getBlockSyncPeers

Get peers in Block Sync module.

### Params

No parameters

### Returns

`string[]`

### Request Example

```
  curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "devel_getBlockSyncPeers", "params": [], "id": 3}' \
    localhost:8080
```

### Response Example

```
{
  "jsonrpc":"2.0",
  "result": ["1.2.3.4:3485", "1.2.3.5:3485"],
  "id":3
}
`````

[Back to **List of methods**](#list-of-methods)
