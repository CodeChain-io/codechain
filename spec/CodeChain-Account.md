An account in CodeChain represents a holder of [CodeChain Coin](CodeChain-Coin.md), and a sender of parcels. The core elements of an account are:

* An identifying address such as XXX
* A sequence number, starting at 1, increases with each parcel sent from this account. No parcel can be included in a ledger unless the parcel’s sequence number matches its sender’s next sequence number.
* One or more ways to authorize parcels, possibly including
  * A master key pair intrinsic to the account
  * Regular key pairs that are explicitly registered
