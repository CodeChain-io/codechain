###########################
Asset Management by Example
###########################

Getting Started
===============

If you want to start creating assets that can be transferred amongst users, you can do it with codechain-sdk-js.
If you visit this link__, you can see an example JavaScript code.
This page will guide you along on how to use codechain-sdk-js based on the example provided,
called “Mint 10000 Gold and send 3000 Gold using AssetMintTransaction, AssetTransferTransaction”.

__ https://api.codechain.io

Before following any examples, please make sure to carefully go through the `setup section <https://codechain.readthedocs.io/en/latest/setup.html>`_ before starting any examples.

Then, check whether your CodeChain RPC server is up and running. You can read about how that is done in the `configure section <https://codechain.readthedocs.io/en/latest/configuration.html>`_.

Setup the Test Account
=====================================
Before you begin with various examples, you need to setup an account. The given account (cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7) holds 100000 CCC at the genesis block. It's a sufficient
quantity to pay for the transaction fee. You can setup the account by using this:
::

    wget https://raw.githubusercontent.com/CodeChain-io/codechain-sdk-js/master/examples/import-test-account.js

If successful, the command line will output the address of the account being used for the transaction fee. In this case, it will output cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7.

Then run the downloaded .js file with the following command:

``node import-test-account.js``

.. note::
    The initial 100000 CCC is only available in test mode.

Running the Sample Assets Minting Code
======================================
Once you have installed codechain-sdk, go to the installed directory and create a JavaScript file with the example code.
For simplicity, we will call this sample script mint-and-transfer.js. To download the .js file, run:
::

    wget https://raw.githubusercontent.com/CodeChain-io/codechain-sdk-js/master/examples/mint-and-transfer.js

Then, run the following command:

``node mint-and-transfer.js``

This should give you the following result:
::

    Asset {
        assetType:
        H256 {
            value: '5300000000000000179399be5182ae43b92acbb9de935000f5e33c23e6d4ceba' },
        lockScriptHash:
        H256 {
            value: 'f42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3' },
        parameters:
        [ [ 208,
            251,
            253,
            21,
            232,
            131,
            214,
            80,
            73,
            177,
            128,
            232,
            250,
            151,
            108,
            210,
            60,
            69,
            101,
            113,
            113,
            130,
            172,
            17,
            195,
            42,
            207,
            229,
            248,
            152,
            159,
            14 ] ],
        quantity: 3000,
        outPoint:
        AssetOutPoint {
            transactionHash:
            H256 {
                value: '5724c9377508058a27b7fbff10d60255a429ef905792986c07571fcaf0fff980' },
            index: 0,
            assetType:
            H256 {
                value: '5300000000000000179399be5182ae43b92acbb9de935000f5e33c23e6d4ceba' },
            quantity: 3000,
            lockScriptHash:
            H256 {
                value: 'f42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3' },
            parameters: [ [Array] ] } }
        Asset {
        assetType:
        H256 {
            value: '5300000000000000179399be5182ae43b92acbb9de935000f5e33c23e6d4ceba' },
        lockScriptHash:
        H256 {
            value: 'f42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3' },
        parameters:
        [ [ 174,
            155,
            53,
            229,
            89,
            202,
            36,
            156,
            33,
            75,
            16,
            147,
            201,
            78,
            224,
            71,
            48,
            132,
            174,
            192,
            113,
            187,
            89,
            29,
            225,
            236,
            112,
            109,
            204,
            115,
            84,
            88 ] ],
        quantity: 7000,
        outPoint:
        AssetOutPoint {
            transactionHash:
            H256 {
                value: '5724c9377508058a27b7fbff10d60255a429ef905792986c07571fcaf0fff980' },
            index: 1,
            assetType:
            H256 {
                value: '5300000000000000179399be5182ae43b92acbb9de935000f5e33c23e6d4ceba' },
            quantity: 7000,
            lockScriptHash:
            H256 {
                value: 'f42a65ea518ba236c08b261c34af0521fa3cd1aa505e1c18980919cb8945f8f3' },
            parameters: [ [Array] ] } }

In this example, 10000 gold has been minted for Alice. Alice then sends 3000 gold to Bob.
Let’s see how all of this works specifically by inspecting parts of the code one by one.

Setting Up Basic Properties
===========================
Make sure you are accessing the CodeChain port. In this example, it is assumed that you are using port 8080, since that is the default value.
::

    const sdk = new SDK({ server: “http://localhost:8080” });

The MemoryKeyStore is created for testing purposes. In real applications, the MemoryKeyStore would be in the form of storage, such as hardware
wallets or the key store server, which would hold and manage the key pair (private and public keys). If you want to use the key store server see below `remote key store`_.
The P2PKH is responsible for locking and unlocking scripts.
::

    const keyStore = await sdk.key.createMemoryKeyStore();
    const p2pkh = await sdk.key.createP2PKH({ keyStore });

Each user needs an address to receive/send assets. Addresses are created by p2pkh. In this example, Bob's address is introduced differently,
since Bob's address is recieved from Bob. In real world applications, you would only know the address of the recipient and nothing more.
::

    const aliceAddress = await p2pkh.createAddress();
    const bobAddress = "ccaqqqap7lazh5g84jsfxccp686jakdy0z9v4chrq4vz8pj4nl9lzvf7rs2rnmc0";

If you want to see Alice's address, run the following:
::

    console.log(aliceAddress.toString());

This will result in showing you an address that is identical to the format of Bob's address shown above.

Minting/Creating New Assets
===========================
In order to create new assets, you must create a new instance of AssetScheme. In this example, we create 10000 gold with the following code:
::

    const goldAssetScheme = sdk.core.createAssetScheme({
        shardId: 0,
        metadata: JSON.stringify({
            name: "Gold",
            description: "An asset example",
            icon_url: "https://gold.image/",
        }),
        supply: 10000,
        approver: null,
    });

.. note::
    You should note that the approver is kept as null. This value is only filled out when there should be an overseer amongst transactions.
    If not null, the approver must approve of every transaction being made with that newly created Asset. In this case, if the 10000 gold
    that was minted had a approver, then every time any of those 10000 gold is involved in a transaction, the set approver would have to
    sign off and approve for the transaction to be successful.

After Gold has been defined in the scheme, the supply that is minted but belong to someone initially. In this example, we create 10000 gold for Alice.
::

    const mintTx = sdk.core.createAssetMintTransaction({
        scheme: goldAssetScheme,
        recipient: aliceAddress

Sending/Transferring Assets
===========================
Alice then sends 3000 gold to Bob. In CodeChain, users must follow the `UTXO <https://codechain.readthedocs.io/en/latest/what-is-codechain.html#what-is-utxo>`_
standard, and make a transaction that spends an entire UTXO balance, and receive the change back through another transaction.

Next, we create an output which gives 3000 gold to Bob, and returns 7000 gold to Alice.
::

    const firstGold = mintTx.getMintedAsset();
    const transferTx = sdk.core.createTransferAssetTransaction()
        .addInputs(firstGold)
        .addOutputs({
            recipient: bobAddress,
            quantity: 3000,
            assetType: firstGold.assetType
        }, {
            recipient: aliceAddress,
            quantity: 7000,
            assetType: firstGold.assetType
        });


By using Alice's signature, the 10000 gold that was first minted can now be transferred to other users like Bob.
::

    await transferTx.sign(0, { signer: p2pkh });
    transferTx.getTransferredAssets();

The transaction containing the Gold asset is sent to the node. The transaction fee is paid for by the account known as
``cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7`` with the passphrase ``satoshi``. 
::

    await sdk.rpc.chain.sendTransaction(transferTransaction, {
        account: "cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7",
        passphrase: "satoshi",
    });


In order to check if all the transactions were successful, we run the following:
::

    // Unspent Bob's 3000 golds
    console.log(await sdk.rpc.chain.getAsset(transferTx.hash(), 0));
    // Unspent Alice's 7000 golds
    console.log(await sdk.rpc.chain.getAsset(transferTx.hash(), 1));

This should return the following:
::

    [RESULTS WILL BE FIXED AND REUPLOADED]

[EXPLANATION WILL BE REVISED]

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

Account ID is a result of ripemd160 of blake256 of a public key (64 bytes uncompressed form).

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

.. _remote key store:

Use RemoteKeyStore to save Asset Address private key
==========================================================

You should use a key management server to use Asset Address private keys safely. You can use a standalone key management server from this link__.
In this section, we will install and run the key management server, and use the server in the SDK.

__ https://github.com/codechain-io/codechain-keystore

Setup the server
-------------------

To run the key management server, nodejs and yarn should be installed.

Clone CodeChain-Keystore repository from the below URL.
::

  git clone https://github.com/CodeChain-io/codechain-keystore-server.git

Move to the directory
::

  cd codechain-keystore

Install the dependencies
::

  yarn install

Run the server
----------------

Below command will run the server
::

  NODE_ENV=production yarn run start

Use the SDK's RemoteKeyStore
--------------------------------

The SDK can use the key management server through ``RemoteKeyStore`` class.
::

  const keyStore = await sdk.key.createRemoteKeyStore("http://<key-management-server-address>");

If you are running the keystore server in the same machine, you can use the ``keyStore`` object instead of the memory keystore. Refer to the example below:
::

  const keyStore = await sdk.key.createRemoteKeyStore("http://127.0.0.1:7007");

Example
-----------

Here is a sample which uses ``RemoteKeyStore`` to create and get accounts. If you run this example multiple times, the number of printed keys is increased every time.
::

  var { RemoteKeyStore } = require("codechain-sdk/lib/key/classes")
  async function main() {
    var keyStore = await RemoteKeyStore.create("http://<key-management-server-address>");
    await keyStore.createKey({ passphrase: "mypassword" });
    var keys = await keyStore.getKeyList();
    console.dir(keys);
  }
  main().catch(err => console.error(err));
