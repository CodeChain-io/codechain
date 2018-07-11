.. _asset-transaction:

##############################
Asset Transactions
##############################
When assets are created, there has to be transactions that change ownership of those assets. However, there is a difference between a transaction that involves
newly minted assets and existing assets.

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
the Asset Transfer Transaction. By using Alice's signature, assets can be send to any user, if their `Asset Address <https://codechain.readthedocs.io/en/latest/asset-management.html#asset-transfer-address-format>`_
is known.