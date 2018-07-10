.. _consensus-algorithms:

#########################
Consensus Algorithms
#########################
Currently there are five consensus algorithms being used in CodeChain. Conensus algorithms each have their strengths,
which is why a variety is being offered.

Solo
==========================
Used for testing purposes only when there is only one node in the entire network. Solo is not a consensus algorithm.

Tendermint
==========================
`Tendermint <https://tendermint.com/>`_ is a Proof-of-Stake algorithm which is designed to tolerate machines that fail in arbitrary ways,
which is also known as Byzantine fault tolerance(BFT). Tendermint claims that even if 1/3 of the machines fail, it will still operate properly,
offering a secure and consistent system.

BlakePoW
==========================
BlakePoW follows the Proof-of-Work model of Bitcoin, where a hash is calculated by adding the nonce and the block hash. It is then checked whether
this added value is less than or equal to the target value over and over again. If you want an algorithm not bound to forms of processing power,
please use Cuckoo.

Cuckoo
==========================
Cuckoo aims to be resistant to Bitcoin style hardware arms-races by making its algorithm memory bound. Thus, solution times should be bound to
memory bandwidth instead of other forms of raw processing power. As a result, Cuckoo should be a viable solution for running on most commodity
hardware, and require far less energy than other forms of PoW algorithms that are bound to GPU, CPU or ASIC.

PoW Mining Difficulty
==========================
Currently, the mining difficulty is adjusted depending on the timestamp differences of the blocks. If the difference is less than 10 seconds,
the difficulty is adjusted upwards. If the timestamp difference is between 10 to 19 seconds, the difficulty is left unchanged. If greater
than or equal to 20 seconds, the difficulty is adjusted downwards proportional to the timestamp difference.

RPC API
==========================
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