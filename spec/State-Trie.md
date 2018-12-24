CodeChain has three kinds of states. The states are `Account`, `AssetScheme` and `Asset`. They are distinguished by their address but they share one global state trie.

# Account

```rust
struct Account {
    balance: U256,
    seq: U256,
    regular_key: Option<Public>
}
```

# AssetScheme

```rust
struct AssetScheme {
    metadata: String,
    amount: u64,
    registrar: Option<Address>,
}
```

# Asset

```rust
struct Asset {
    asset_type: H256,
    script_hash: H256,
    parameters: Vec<Bytes>,
    amount: u64,
}
```

asset_type = BLAKE2b(OutPoint pointing to AssetMint)[0..32].
