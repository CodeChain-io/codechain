# Curve

CodeChain uses [secp256k1](http://www.secg.org/sec2-v2.pdf) because it is the most popular elliptic curve parameters used by both Bitcoin and Ethereum.

# Signature Algorithm

CodeChain uses [Schnorr signature](https://en.wikipedia.org/wiki/Schnorr_signature) as its digital signature algorithm instead of more conventional ECDSA.

Schnorr signature has a couple of nice properties:

1. Efficient threshold signatures for n-of-n. Multiple Schnorr signatures can be combined to end up with a signature valid for the sum of the public keys, so arbitrarily large n-of-n multisigs can be done by only communicating the single sum signature, which can be verified with a single CHECKSIG operation.
1. The size of data to be validated and transmitted have been diminished due to smaller signatures (64 bytes instead of 71-72) with none of the problems that DER encodings have caused for Bitcoin. 
1. Potential support for batch validation (up to a factor 2 speedup to verify groups of 32 signatures at once). This requires knowing the R.y coordinate (ECDSA ignores this) and at the script level, guaranteeing that all signature verification failure results in script failure (i.e., all CHECKSIG operators behave like CHECKSIGVERIFY). 
1. Stronger security proof: Provably no inherent signature malleability, while ECDSA has a known malleability, and lacks proof that no other forms exist.
1. Slightly faster to sign/verify than ECDSA.
