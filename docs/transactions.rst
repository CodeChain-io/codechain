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
===========================
(to be completed)

Asset Transfer Transaction
===========================
(to be completed)

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