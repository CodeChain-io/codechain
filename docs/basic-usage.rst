Basic Usage
###########

Run Built Executable
====================
To get started, you must first run the built executable of CodeChain.

In order to run CodeChain, run
::

    ./target/release/codechain

Currently only operates in standalone mode, and you can create a block by sending a parcel through `JSON-RPC <https://github.com/CodeChain-io/codechain/wiki/JSON-RPC>`_ or `JavaScript SDK <https://api.codechain.io/>`_.

Blockchain Configuration
========================
When configuring CodeChain's blockchain type, you can set it to either ``Solo``, ``Solo-Authority`` or ``Tendermint``. 

Solo Configuration
------------------


Solo-Authority Configuration
----------------------------


Tendermint Configuration
------------------------
In order to properly get Tendermint to get going, you need to have 4 nodes up and running. To do this, first run a single node.
Then create more nodes, and allocate each node with a secret key that corresponds to one of the four public keys listed in Tendermint's validator property.
Once each public key has a corresponding node with a corresponding secret key, use the boostrap address command to interlink all the nodes together.
The way each node is connected does not matter, as long as each node is connected to another node. 

Checking if CodeChain is Configured Properly
============================================
JSON-RPC is a stateless, light-weight remote procedure call (RPC) protocol. Primarily this specification defines several data structures and the rules 
around their processing. It is transport agnostic in that the concepts can be used within the same process, over sockets, over HTTP, or in many various 
message passing environments. It uses JSON (RFC 4627) as data format.


Using Curl
----------
First, check whether CodeChain's RPC port is listening for RPC connections. By default it should be PORT 8080.

In order to check whether CodeChain is configured properly or not, send a ping to check whether CodeChain's RPC server is actually responding. To do this, do the following:
::

    curl \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc": "2.0", "method": "ping", "params": [], "id": null}' \
        localhost:8080

You should get the following response, or something similar:
::

    {"jsonrpc":"2.0","result":"pong","id":null}

Using JavaScript SDK
--------------------
In order to use this method, first install the sdk by running the following:
::

    npm install codechain-sdk

or
::

    yarn add codechain-sdk

Then, make sure that your CodeChain RPC server is listening. In the examples, we assume it is localhost:8080

If you run the following code, your should receive a ping response:
::

    // ping.js (javascript)
    var SDK = require("codechain-sdk");

    var sdk = new SDK("http://localhost:8080");

    sdk.ping().then(function (response) {
        console.log("Ping response:", response);
    }).catch(console.error);