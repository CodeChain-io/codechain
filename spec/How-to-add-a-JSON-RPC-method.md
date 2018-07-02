## 1. Choose a module that your method will be placed in.

 * **chain** module is for accessing the blockchain and the parcel queue.
 * **devel** module is for utility functions to debug CodeChain

## 2. Modify `rpc/src/v1/traits/<module name>.rs`

`chain_getBlockHash` example ([chain_getBlockHash usage example](https://github.com/CodeChain-io/codechain/wiki/JSON-RPC#chain_getblockhash)):
```
/// Gets the hash of the block with given number.
# [rpc(name = "chain_getBlockHash")]
fn get_block_hash(&self, u64) -> Result<Option<H256>>;
```

The above declaration is a part of the `pub trait Chain`. It creates the RPC endpoint to "chain_getBlockHash" which receives `u64` parameter. All of the parameters must be serde-Serializable. See files in the `src/rpc/v1/types/` directory.

## 3. Modify `rpc/src/v1/impls/<module name>.rs`

`chain_getBlockHash` example:
```
fn get_block_hash(&self, block_number: u64) -> Result<Option<H256>> {
    Ok(self.client.block_hash(BlockId::Number(block_number)))
}
```

The above implementation is a part of the `impl Chain for ChainClient`. ChainClient holds `Client` and `Miner` which are structs for both the blockchain and the parcel queue.

The above shows `chain_getBlockHash`, which is implemented using the `block_hash` function in `Client`.