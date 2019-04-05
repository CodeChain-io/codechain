Basic Usage
###########

Run Built Executable
====================
To get started, you must first run the built executable of CodeChain.

In order to run CodeChain, run
::

    ./target/release/codechain

You can create a block by sending a transaction through `JSON-RPC <https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md>`_. In order to utilize
JSON-RPC, you can use Curl or `JavaScript SDK <https://api.codechain.io/>`_.

Blockchain Configuration
========================
You can run your network using ``Solo`` or ``Tendermint`` consensus, or you can join an existing network.

Solo Configuration
------------------
CodeChain uses this configuration as default. In order to change it into another configuration, such as tendermint, run:
::

    --chain tendermint

Tendermint Configuration
------------------------
In order to properly get Tendermint to get going, you need to have 4 nodes up and running. To do this, first run a single node by running the following:
::

    codechain --db-path db/db0 --port 3485 --jsonrpc-port 8080 --engine-signer tccqzzpxln6w5zrhmfju3zc53w6w4y6s95mf5hw0n62 -c tendermint

This creates a node in db0 (database 0) at port 3485 (used for nodes to communicate with each other) and jsonRPC port 8080 (port used for external access) with engine signer of tccqzzpxln6w5zrhmfju3zc53w6w4y6s95mf5hw0n62 (used to sign the block).

Then create more nodes, and allocate each node with a secret key that corresponds to one of the four public keys listed in Tendermint's validator property.
When creating new nodes, the db, port and jsonRPC port all must be configured as a different value. So for example, the next node should be set up like this:
::

    codechain --db-path db/db1 --port 3486 --jsonrpc-port 8081 --engine-signer tccqz03jn96q0kvwqzxgeq5u72e2l8v5vkdyq4cll9x -c tendermint

Once each public key has a corresponding node with a corresponding secret key, use the boostrap address command to interlink all the nodes together.
The way each node is connected does not matter, as long as each node is connected to another node. For example, in order to make a certain node connect to
the node with a secret key of 1, use this command:
::

    codechain --db-path db/db1 --port 3486 --jsonrpc-port 8081 --engine-signer tccqr8a9rqj09j9l6ahe7yq9xfjj8h5xw3p7vpcgner -c tendermint --bootstrap-addresses 127.0.0.1:3485

Connect to the existing network
--------------------------------
You can participate in the Corgi or Main network.

You could get information about Corgi at this `link <https://corgi.codechain.io/>`_.
In order to participate in the Corgi network, you should use the command below:
::

    codechain --chain corgi --no-miner --bootstrap-addresses "52.68.160.158:3485" "52.87.80.242:3485" "13.52.125.202:3485" "18.184.72.190:3485" "13.124.7.55:3485"

In order to participate in the Main network, you should use the command below:
::

    codechain --no-miner --bootstrap-addresses "13.115.159.65:3485" "18.205.137.116:3485" "13.52.129.93:3485" "18.194.21.237:3485" "13.124.155.240:3485"

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

    var sdk = new SDK({ server: "http://localhost:8080" });

    sdk.rpc.node.ping().then(function (response) {
        console.log("Ping response:", response);
    }).catch(console.error);

If you want to run the above example in the command line, first install ``nvm`` by running the following:
::

    wget -qO- https://raw.githubusercontent.com/creationix/nvm/v0.33.11/install.sh | bash

Then run the following:
::

    node -e 'var SDK = require("codechain-sdk"); var sdk = new SDK({ server: "http://localhost:8080" });sdk.rpc.node.ping().then(function (response) {console.log("Ping response:", response); }).catch(console.error);'

You should receive the following response:
::

    Ping response: pong
