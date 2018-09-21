# Parcel

A parcel is a group of transactions or a transaction about platform account. Only a platform account can generate a parcel. The parcel generator pays the transaction fees.

The nonce must be identical with the sender’s account nonce. The account nonce will be increased by 1 after a parcel is added to the block. The amount fee is deducted from the sender account’s balance. A parcel will not be included if the nonce of the account doesn’t match or the balance of the account is less than the fee.

A parcel expires if a block is validated after the parcel's set expiration time. In addition, parcels can also have lock times. A block's current timestamp must be later than the parcel's lock time for the parcel to be included in the block.

```rust
struct Parcel {
    version: u64,
    expiration_time: Option<Timestamp>,
    nonce: U256,
    fee: U256,
    network_id: NetworkId,
    action: Action,
}

enum Action {
    AssetTransactionGroup { ..., },
    Payment { ..., },
    SetRegularKey { ..., },
}
```

## AssetTransactionGroup

Execute `transactions`. If `block_num` is specified, parcel is valid only in block whose number is in range of [block_num, block_num + margin).

```rust
AssetTransactionGroup {
    block_num: Option<BlockNumber>
    transactions: Vec<Transaction>
}
```

## Payment

`Payment` parcel sends `value` amount of CCC to the `receiver`.

```rust
Payment {
    receiver: Address,
    value: U256,
}
```

## SetRegularKey

`SetRegularKey` parcel sets the regular `key` of the parcel sender. It overwrites the existing one if a key already exists.

```rust
SetRegularKey {
    key: Public,
}
```

# Transaction

```rust
enum Transaction {
    AssetMint { ..., },
    AssetTransfer { ..., },
}
```

## AssetMint

```rust
AssetMint {
    network_id: NetworkId,
    shard_id: u32,
    metadata: String,
    registrar: Option<Address>,
    nonce: u32,
    output: AssetMintOutput
}

struct AssetMintOutput {
    lock_script_hash: H256,
    parameters: Vec<Bytes>,
    amount: u64,
}
```

When an asset is marked as permissioned, `AssetTransfer` transactions must include the `registrar`'s signature.

## AssetTransfer

```rust
AssetTransfer {
    network_id: NetworkId,
    burns: Vec<AssetTransferInput>,
    inputs: Vec<AssetTransferInput>,
    outputs: Vec<AssetTransferOutput>
    nonce: u32,
}

struct AssetTransferInput {
    prev_out: AssetOutPoint,
    timelock: Option<Timelock>,
    lock_script: Script,
    unlock_script: Script,
}
struct AssetOutPoint {
    transaction_hash: H256,
    index: usize,
    asset_type: H256,
    amount: u64,
}
struct AssetTransferOutput {
    lock_script_hash: H256,
    parameters: Vec<Bytes>,
    asset_type: H256,
    amount: u64,
}
```

### Timelock

AssetTransferTransaction fails if the `timelock` condition isn't met. There are 4 types for `timelock`. Basically, they keep the transaction from being executed until the specific point in time. `Block` and `Time` types indicate the absolute time. `BlockAge` and `TimeAge` types indicate relative time based on how long has the asset been created.

- `Block(u64)`: The given value must be equal or greater then the number of the current block.
- `BlockAge(u64)`: The given value must be equal or greater then the value which is the current block number minus the block number which the asset was created in.
- `Time(u64)`: The given value must be equal or greater then the timestamp of the current block.
- `TimeAge(u64)`: The given value must be equal or grater then the value which is the current block timestamp minus the timestamp of the block which the asset was created in.

```rust
enum Timelock {
    Block(u64),
    BlockAge(u64),
    Time(u64),
    TimeAge(u64),
}
```
