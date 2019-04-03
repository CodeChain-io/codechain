# Transaction

Only a platform account can generate a transaction.
The transaction generator pays the transaction fees.

The seq must be identical with the payer’s account seq.
The account seq will be increased by 1 after a transaction is added to the block.
The amount of fee is deducted from the payer’s balance.
A transaction will not be included if the seq of the account doesn’t match or the balance of the account is less than the fee.

```rust
struct Transaction {
    seq: u64,
    fee: u64,
    network_id: NetworkId,
    action: Action,
}

enum Action {
    MintAsset { ..., },
    TransferAsset { ..., },
    ChangeAssetScheme { ..., },
    Pay { ..., },
    SetRegularKey { ..., },
    WrapCCC { ..., },
    UnwrapCCC { ..., },
    Store { ..., },
    Remove { ..., },
    Custom { ..., },
}
```

## MintAsset

`MintAsset` issues a new asset and an asset scheme that goes along with it.
The output becomes the lock script hash and parameters of the new asset.

A permissioned asset is an asset that has an approver.
This kind of asset needs permission to be transferred.

A regulated asset is an asset that has an registrar.
The registrar can change the asset scheme and transfer the asset arbitrarily.

```rust
MintAsset {
    network_id: NetworkId,
    shard_id: ShardId,
    metadata: String,
    approver: Option<PlatformAddress>,
    registrar: Option<PlatformAddress>,

    output: AssetMintOutput,

    approvals: Vec<Signature>,
}

struct AssetMintOutput {
    lock_script_hash: H160,
    parameters: Vec<Bytes>,
    amount: u64,
}
```

## TransferAsset

It transfers assets.
The transfer must provide the valid lock_script and unlock_script.

```rust
TransferAsset {
    network_id: NetworkId,
    burns: Vec<AssetTransferInput>,
    inputs: Vec<AssetTransferInput>,
    outputs: Vec<AssetTransferOutput>,
    orders: Vec<OrderOnTransfer>,

    metadata: String,
    approvals: Vec<Signature>,
    expiration: Option<u64>,
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
    lock_script_hash: H160,
    parameters: Vec<Bytes>,
    asset_type: H256,
    amount: u64,
}
```

### Timelock

A transaction fails if any `timelock` condition isn't met.
There are 4 types of `timelock`.
Basically, they keep the transaction from being executed until the specific point in time.
`Block` and `Time` types indicate the absolute time.
`BlockAge` and `TimeAge` types indicate relative time based on how long has the asset been created.

- `Block(u64)`: The given value must be less than or equal to the current block's number.
- `BlockAge(u64)`: The given value must be less than or equal to the value `X`, where `X` = `current block number` - `the block number that the asset of the AssetOutPoint was created at`.
- `Time(u64)`: The given value must be less than or equal to the current block's timestamp.
- `TimeAge(u64)`: The given value must be less than or equal to the value `X`, where `X` = `current block timestamp` - `the block timestamp that the asset of the AssetOutPoint was created at`.

```rust
enum Timelock {
    Block(u64),
    BlockAge(u64),
    Time(u64),
    TimeAge(u64),
}
```

### Order

Order is used for the DEX.
Please see [this page](./Asset-Exchange-Protocol.md) for more information.

## ChangeAssetScheme

It changes the asset scheme.
Only the registrar of the asset can use it.

```rust
ChangeAssetScheme {
    network_id: NetworkId,
    asset_type: H256,
    metadata: String,
    approver: Option<PlatformAddress>,
    registrar: Option<PlatformAddress>,

    approvals: Vec<Signature>,
}
```

## Pay

`Pay` sends `value` amount of CCC to the `receiver`.

```rust
Pay {
    receiver: Address,
    amount: u64,
}
```

## SetRegularKey

`SetRegularKey` sets the regular `key` of the payer.
It overwrites the existing one if a key already exists.

```rust
SetRegularKey {
    key: Public,
}
```

## WrapCCC

`WrapCCC` converts CCC to WCCC.
The payer must own enough CCC to convert.
```rust
WrapCCC {
    shard_id: ShardId,
    lock_script_hash: H160,
    parameters: Vec<Parameters>,
    amount: u64,
}
```

## UnwrapCCC

`UnwrapCCC` converts WCCC to CCC.
The payer has the converted CCC.

```rust
UnwrapCCC {
    network_id: NetworkId,
    burn: AssetTransferInput,
    receiver: PlatformAddress,
}
```

```rust
Custom {
    handler_id: u64,
    bytes: Bytes,
}
```

## Store

This is a special kind of transaction that allows a user to upload text onto the blockchain.

```rust
Store {
    content: String,
    certifier: Address,
    signature: Signature,
}
```

## Remove

It removes the content created by the `Store` transaction.

```rust
Remove {
    hash: H256,
    signature: Signature,
}
```

## Custom

`Custom` is a special transaction.
The types of transactions that may exist depends on the consensus engine.
