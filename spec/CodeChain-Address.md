When transfer CCC or assets, the sender must know the recipient's lock script hash and parameters. An address is a converted form of lock script hash and parameters, and it has some benefits.

 * Address includes a checksum. It's a high probability that a mistyped address is invalid.
 * Address is case-insensitive alphanumeric which is easy to speak aloud or type on the mobile phone. It also efficient to generate QR code.

## Bech32

CodeChain adopted [Bitcoin's Bech32 Specification](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32). The differences from Bitcoin are the followings:

 * CodeChain has no separator
 * CodeChain has 2 types of address. One is for CCC and the other is for assets. They are distinguished by HRP(Human Readable Part)

Address formats are not a core part.

## 1. Platform Account Address Format

HRP: `"ccc"` for Mainnet, `"tcc"` for Testnet.

Data Part: `version` . `body`

### Version 0 (0x00)

Data body: `Account ID` (20 bytes)

Account ID is a result of ripemd160 of blake256 of a public key(64 bytes uncompressed form).

## 2. Asset Transfer Address Format

HRP: `"cca"` for Mainnet, `"tca"` for Testnet.

Data: `version` . `body` 

### Version 0 (0x00)

Data body: `type` . `payload`

#### Type 0 (0x00)

Payload: \<LockScriptHash> (32 bytes)

Type 0 with given payload represents:
 * Lock Script Hash: \<LockScriptHash>
 * Parameters: []

#### Type 1 (0x01)

Payload: \<Public Key Hash> (32 bytes)

Type 1 with given payload represents:
 * Lock Script Hash: P2PKH Standard Script Hash
 * Parameters: [\<Public Key Hash>]


---

Examples will be added soon.