CodeChain uses the [BLAKE2](https://blake2.net/) hash function.

SHA256, used in Bitcoin, has a number of technical shortcomings due to its Merkle-Damgård construction. These vulnerabilities led to the SHA3 competition for a new hash function based on a different fundamental construction.

CodeChain has chosen BLAKE-256 as its hash function, a finalist for the competition. The hash function is based around a HAIFA construction that incorporates a variation of the ChaCha stream cipher by Bernstein. The hash function is notable for its high performance on x86-64 microarchitecture, being faster for short messages than SHA256 despite being considered to have a much higher security margin at 14-rounds.
