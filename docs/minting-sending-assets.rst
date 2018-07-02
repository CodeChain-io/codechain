#####################
Asset Management
#####################

Getting Started
===============

If you want to start creating assets that can be transferred amongst users, you can do it with codechain-sdk-js. 
If you visit this `link <https://api.codechain.io>`_, you can see an example JavaScript code. 
This page will guide you along on how to use codechain-sdk-js based on the example provided, 
called “Mint 10000 Gold and send 3000 Gold using AssetMintTransaction, AssetTransferTransaction”.

Before starting, please install node.js by going to this `page <https://nodejs.org/en/>`_.

First, install the package with the following command:

``npm install codechain-sdk`` or ``yarn add codechain-sdk``

Then, make sure that your CodeChain RPC server is up and running. You can read about how that is done in the `configure section <https://codechain.readthedocs.io/en/latest/configuration.html>`_.

Running the Sample Assets Minting Code
======================================
Once you have installed codechain-sdk, go to the installed directory and create a JavaScript file with the example code. 
For simplicity, we will call this sample script sampleScript.js. Then, run the following command:

``node sampleTransfer.js``

This should give you the following result:
::

    Asset {
    assetType:
    H256 {
        value: '53000000000000009364bc7d89c5a424c1367e280cefc86461624fedb306fc59' },
    lockScriptHash:
    H256 {
        value: '0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073' },
        parameters: [],
        amount: 10000 }
    AssetScheme {
    metadata: '{"name":"Gold","imageUrl":"https://gold.image/"}',
    registrar: null,
    amount: 10000 }
    null
    Asset {
    assetType:
    H256 {
        value: '53000000000000009364bc7d89c5a424c1367e280cefc86461624fedb306fc59' },
    lockScriptHash:
        H256 {
        value: '92e9b25eed924b5b17268934798c0c70f66de38bda64b480012de9be57ac4ec1' },
        parameters: [],
        amount: 3000 }
    Asset {
    assetType:
    H256 {
        value: '53000000000000009364bc7d89c5a424c1367e280cefc86461624fedb306fc59' },
    lockScriptHash:
    H256 {
        value: '0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073' },
        parameters: [],
        amount: 7000 }


In this example, 10000 gold has been minted for Alice. Alice then basically sends 3000 gold to Bob. 
Let’s see how all of this works specifically by inspecting parts of the code one by one.

Setting Up Basic Properties
===========================
Make sure you are accessing the CodeChain port. In this example, it is assumed that you are using port 8080, since that is the default value.
::

    const sdk = new SDK("http://localhost:8080");

Next, set Alice and Bob’s private and public keys. LockScripts, which are made out of public keys, are needed to unlock transactions.
::

    // CodeChain opcodes for P2PK(Pay to Public Key)
    const OP_PUSHB = 0x32;
    const OP_CHECKSIG = 0x80;

    // Alice's key pair
    const alicePrivate = "37a948d2e9ae622f3b9e224657249259312ffd3f2d105eabda6f222074608df3";
    const alicePublic = privateKeyToPublic(alicePrivate);
    // Alice's P2PK script
    const aliceLockScript = Buffer.from([OP_PUSHB, 64, ...Buffer.from(alicePublic, "hex"), OP_CHECKSIG]);

    // Bob's key pair
    const bobPrivate = "f9387b3247c21e88c656490914f4598a3b52b807517753b4a9d7a51d54a6260c";
    const bobPublic = privateKeyToPublic(bobPrivate);
    // Bob's P2PK script
    const bobLockScript = Buffer.from([OP_PUSHB, 64, ...Buffer.from(bobPublic, "hex"), OP_CHECKSIG]);

Minting/Creating New Assets
===========================
In order to create new assets, you must create a new instance of AssetMintTransaction. In this example, we create 10000 gold with the following code:
::

    // Create AssetMintTransaction that creates 10000 amount of asset named Gold for Alice.
    const mintGoldTx = new AssetMintTransaction({
       // Put name, description and imageUrl to metadata.
       metadata: JSON.stringify({
           name: "Gold",
           imageUrl: "https://gold.image/"
       }),
       // hash value of locking script of the asset
       lockScriptHash: new H256(blake256(aliceLockScript)), //shows ownership
       parameters: [], //shows ownership
       // Mints 10000 golds
       amount: 10000,
       // No registrar for Gold. It means AssetTransfer of Gold can be done with any
       // parcel. If registrar is present, the parcel must be signed with the
       // registrar.
       registrar: null, //if not null, the creator must allow this transaction
       nonce: 0
    });

.. note::
    You should note that the registrar is kept as null. This value is only filled out when there should be an overseer amongst transactions. 
    If not null, the registrar must approve of every transaction being made with that newly created Asset. In this case, if the 10000 gold 
    that was minted had a registrar, then every time any of those 10000 gold is involved in a transaction, the set registrar would have to 
    sign off and approve for the transaction to be successful. 

Sending/Transferring Assets
===========================
In this example, in order for Alice to send 3000 gold to Bob, she must first input all of her 10000 gold into a transaction. 
According to UTXO, a spender must spend all of his/her assets first, even if he/she wants to use a partial amount, and receive remainder back later.
::

    // Create an input that spends 10000 golds
    const inputs = [new AssetTransferInput({
       prevOut: new AssetOutPoint({
           transactionHash: mintGoldTx.hash(),
           index: 0,
           assetType: goldAssetType,
           amount: 10000
       }),
       // Provide the preimage of the lockScriptHash.
       lockScript: aliceLockScript,
       // unlockScript can't be calculated at this moment.
       unlockScript: Buffer.from([])
    })];

Next, we create an output which gives 3000 gold to Bob, and returns 7000 gold to Alice.
::

    // Create outputs. The sum of amount must equals to 10000. In this case, Alice
    // pays 3000 golds to Bob. Alice is paid the remains back.
    const outputs = [new AssetTransferOutput({
       lockScriptHash: new H256(blake256(bobLockScript)), //shows ownership to bob
       parameters: [],
       assetType: goldAssetType,
       amount: 3000
    }), new AssetTransferOutput({
       lockScriptHash: new H256(blake256(aliceLockScript)), //shows ownership to alice
       parameters: [],
       assetType: goldAssetType,
       amount: 7000
    })];

In order to check if all the transactions were successful, we run the following:
::

    console.log(await sdk.getAsset(mintGoldTx.hash(), 0));

    // Unspent Bob's 3000 golds
    console.log(await sdk.getAsset(transferTx.hash(), 0));
    // Unspent Alice's 7000 golds
    console.log(await sdk.getAsset(transferTx.hash(), 1));

This should return the following:
::

    Alice's lock script hash:  0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073
    Alice's address:  ccaqqqqt970nme6knrpya9rr9elc34r2505gcqxdrhm5e7yka2djqr7quczzktzj
    Bob's lock script hash:  92e9b25eed924b5b17268934798c0c70f66de38bda64b480012de9be57ac4ec1
    Bob's address:  ccaqqqf96djtmkeyj6mzungjdre3sx8panduw9a5e95sqqjm6d727kyasgznna6v
    minted asset scheme:  AssetScheme {
    metadata: '{"name":"Gold","imageUrl":"https://gold.image/"}',
    registrar: null,
    amount: 10000 }
    alice's gold:  Asset {
    assetType:
    H256 {
        value: '53000000000000009364bc7d89c5a424c1367e280cefc86461624fedb306fc59' },
    lockScriptHash:
    H256 {
        value: '0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073' },
    parameters: [],
    amount: 10000,
    outPoint:
    AssetOutPoint {
        data:
        { transactionHash: [Object],
            index: 0,
            assetType: [Object],
            amount: 10000 } } }
    Asset {
    assetType:
    H256 {
        value: '53000000000000009364bc7d89c5a424c1367e280cefc86461624fedb306fc59' },
    lockScriptHash:
    H256 {
        value: '0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073' },
    parameters: [],
    amount: 10000,
    outPoint:
    AssetOutPoint {
        data:
        { transactionHash: [Object],
            index: 0,
            assetType: [Object],
            amount: 10000 } } }
    null
    null


The results show that 7000 gold went to ``0597cf9ef3ab4c61274a31973fc46a3551f44600668efba67c4b754d9007e073`` and 
that 3000 gold went to ``92e9b25eed924b5b17268934798c0c70f66de38bda64b480012de9be57ac4ec1``.


These are the values of each individual’s LockScripts that went through the blake256 hash function. 
If you run each individual’s LockScript under blake256 yourself, you will find that it corresponds to the rightful owners of the assets. 

Address Format
=================================
CodeChain adopted `Bitcoin's Bech32 Specification <https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32>`_. However,
there are differences. Codechain does not have a seperator. Also, there are two types of addresses used in CodeChain, which are 
Platform Address and Asset Address. Platform Addresses are used for CCC, while Asset Addresses are used 
for mintable assets. These addresses have a human readable part, followed by code. Platform Addresses have a ``"ccc"`` tag, while
Asset Addresses have a ``"cca"`` tag.

Platform Account Address Format
------------------------------------
HRP: ``"ccc"`` for Mainnet, ``"tcc"`` for Testnet.

Data Part: ``version`` . ``body``

**Version 0 (0x00)**
Data body: ``Account ID`` (20 bytes)

Account ID is a result of ripemd160 of blake256 of a public key(64 bytes uncompressed form).


Asset Transfer Address Format
------------------------------------
HRP: ``"cca"`` for Mainnet, ``"tca"`` for Testnet.

Data: ``version`` . ``body``

**Version 0 (0x00)**
Data body: ``type`` . ``payload``

Type 0 (0x00)
Payload: <LockScriptHash> (32 bytes)

Type 0 with given payload represents:

Lock Script Hash: <LockScriptHash>
Parameters: []
Type 1 (0x01)
Payload: <Public Key Hash> (32 bytes)

Type 1 with given payload represents:

Lock Script Hash: P2PKH Standard Script Hash
Parameters: [<Public Key Hash>]
