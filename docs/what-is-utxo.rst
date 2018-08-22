.. _what-is-utxo:

#############################
What is UTXO?
#############################
UTXO is an acronym for Unspent Transaction Outputs, which always requires users spend their entire balance defined in a UTXO first, and then receive
the unspent amount back. For instance, if you have a UTXO that defines that you have 10 potions, and you want to buy something that costs 2 potions, you would make a
transaction that would "spend" your entire UTXO balance by sending 2 potions to the other person, and 8 potions back to yourself. Once this transaction is
complete, a UTXO would be created, both for the spender and the receiver. In general, the UTXO specifies how much the user got back or received, which basically defines how much
the user can spend. The amount the user gets back would be added to his/her account balance. Thus, it is most likely that each user would
have more than one UTXOs, and the sum of all the unspent coins in every UTXO would be the user's total account balance.
