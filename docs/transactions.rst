.. _transactions:

#########################
Transactions
#########################

What is UTXO?
==============================
A UTXO is an acronym for Unspent Transaction Outputs, which always requires users to first spend their entire balance defined in a UTXO, and then receving
the unspent amount back. For instance, if you have a UTXO that defines that you have 10 potions, and you want to buy something that costs 2 potions, you would make a
transaction that would "spend" your entire UTXO balance by sending 2 potions to the other person, and 8 potions back to yourself. Once this transaction is
complete, a UTXO would be created, both for the spender and the receiver. In general, the UTXO specifies how much the user got back or received, which basically defines how much
the user can spend. The amount the user gets back would be added to his/her account balance. Thus, it is most likely that each user would
have more than one UTXOs, and the sum of all the unspent coins in every UTXOs would be the user's total account balance.

Asset Mint Transaction
==============================
When assets are newly minted, there are a couple of things you must understand. First, the asset's scheme must be defined, since the asset being created must have some
sort of definition. Second, there must be an owner to this newly minted asset. Thus, when creating assets in CodeChain, an address of the owner is required. A transaction
that sends fresh minted assets to a user is called the `Asset Mint Transaction <https://codechain.readthedocs.io/en/latest/asset-management.html#minting-creating-new-assets>`_.
The address used for Asset Mint Transactions should follow the `Address Format <https://codechain.readthedocs.io/en/latest/asset-management.html#address-format>`_.

Asset Transfer Transaction
==============================
Once assets have been successfully minted, these assets can now be sent to other users. For instance, let's say that the initial owner of the newly minted assets
is Alice. If Alice wants to send some assets to Bob, then a transaction must be created. This transaction of sending assets from one user to another is called
the Asset Transfer Transaction. By using Alice's signature, assets can be send to any user, if their `Asset Address<https://codechain.readthedocs.io/en/latest/asset-management.html#asset-transfer-address-format>`_
is known.

Lock Script
==============================
Lock scripts are required in CodeChain when making a transaction to a different user. When attempting to
make a transaction, the sender must know the receiver's lock script so that the receiver can use his/her
private key to use/spend the newly received asset. This is analagous to sending money to someone's bank
account. Without knowing the reciever's bank account address, you cannot send money to the proper destination.
Lock scripts are contain two parts: the lockScriptHash and parameter.

How are Lock Scripts Created?
==============================
When the user wants to receive any asset, he/she must create a private and public key pair.
The public key is then used to create a lock script that the user needs so that he/she can
receive assets. The codechain-sdk allows the lock scripts to be in a form of an address. This
address is fundamentally a bank address in the real world. Addresses can be decoded to reveal
a user's lockScriptHash and the parameter required to send a transaction.