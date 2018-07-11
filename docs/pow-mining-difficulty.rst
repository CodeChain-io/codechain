.. _pow-mining-difficulty:

#############################
PoW Mining Difficulty
#############################
Currently, the mining difficulty is adjusted depending on the timestamp differences of the blocks. If the difference is less than 10 seconds,
the difficulty is adjusted upwards. If the timestamp difference is between 10 to 19 seconds, the difficulty is left unchanged. If greater
than or equal to 20 seconds, the difficulty is adjusted downwards proportional to the timestamp difference.