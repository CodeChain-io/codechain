.. _consensus-algorithms:

#########################
Consensus Algorithms
#########################

CodeChain offers a pluggable consensus model, which provides flexibility. You can choose the consensus model that best suits your needs. If the existing consensus models do not meet your business requirements, you can easily create your own consensus model.

Currently, CodeChain supports four consensus algorithms. Each consensus algorithm has its own strengths,
which is why a variety is being offered.

.. toctree::
    :maxdepth: 2

    solo
    tendermint
    blakepow
    cuckoo

#############################
PoW Mining Difficulty
#############################

Both BlakePow and Cuckoo are PoW-based consensus models. The mining difficulty is adjusted depending on the timestamp differences of the blocks. If the difference is less than 10 seconds, the difficulty is adjusted upwards. If the timestamp difference is between 10 to 19 seconds, the difficulty is left unchanged. If greater than or equal to 20 seconds, the difficulty is adjusted downwards proportional to the timestamp difference. 
