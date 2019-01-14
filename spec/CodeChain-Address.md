When transferring CCC or assets, the sender must know the recipient's lock script hash and parameters. An address is a converted form of the lock script hash and parameters, and it has some benefits.

 * An address includes a checksum. There is a high probability that a mistyped address is invalid.
 * An address is case-insensitive alphanumeric, which is easy to speak aloud or type on the mobile phone. It also makes it efficient to generate QR codes.

## Bech32

CodeChain adopted [Bitcoin's Bech32 Specification](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32). The differences from Bitcoin are the following:

 * CodeChain has no separator.
 * CodeChain has 2 types of address. One is for CCCs and the other is for assets. They are distinguished by HRP(Human Readable Part)

Address formats are not a core part.

## 1. Platform Account Address Format

HRP: `"ccc"` for Mainnet, `"tcc"` for Testnet.

Data Part: `version` . `body`

### Version 0 (0x00)

No longer available. Any version 0 address will be rejected in the latest clients.

### Version 1 (0x01)

Data body: `Account ID` (20 bytes)

Account ID is the result of blake160 over a public key(64 bytes uncompressed form).

## 2. Asset Transfer Address Format

HRP: `"cca"` for Mainnet, `"tca"` for Testnet.

Data: `version` . `body` 

### Version 0 (0x00)

No longer available. Any version 0 address will be rejected in the latest clients.

### Version 1 (0x01)

Data body: `type` . `payload`

#### Type 0 (0x00)

Payload: \<LockScriptHash> (20 bytes)

Type 0 with given payload represents:
 * Lock Script Hash: \<LockScriptHash>
 * Parameters: []

#### Type 1 (0x01)

Payload: \<Public Key Hash> (20 bytes)

Type 1 with the given payload represents:
 * Lock Script Hash: P2PKH Standard Script Hash (5f5960a7bca6ceeeb0c97bc717562914e7a1de04)
 * Parameters: [\<Public Key Hash>]

#### Type 2 (0x02)

Payload: \<Public Key Hash> (20 bytes)

Type 2 with the given payload represents:
 * Lock Script Hash: P2PKHBurn Standard Script Hash (37572bdcc22d39a59c0d12d301f6271ba3fdd451)
 * Parameters: [\<Public Key Hash>]

---

## Address examples

* Platform Account Address: `cccqx37a03l3axrz3qmtdywgjuyuvr099dueuqvjxp3`
  * version = `1`
  * payload(Account ID) = `a3eebe3f8f4c31441b5b48e44b84e306f295bccf`

* Asset Transfer Address: `ccaqypf8czlf67sds30ylddl4hxzcr7lml73wqqypt3ua`
  * version = `1`
  * type = `2`
  * payload(blake160 of public key) = `93e05f4ebd06c22f27dadfd6e61607efeffe8b80`
