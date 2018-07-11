.. _rpc-api:

#############################
RPC API
#############################
For examples, please click `here <https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md#miner_getwork>`_.

``miner_getWork``

Returns the hash of the current block, the score and the block number.

Params: No parameters

Return Type: work object

``miner_submitWork``

Used for submitting a proof-of-work solution.

Params:

powHash: string
seal: Array of string
Return Type: bool