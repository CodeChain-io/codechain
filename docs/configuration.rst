Configuration
#############

CodeChain can be configured with either CLI options or a config file. When it comes to which options take precedence, it goes from CLI, user's own config.toml file, and config.dev.toml in that order.

CLI options can be listed by running the command ``$codechain --help``. By using the CLI options, or custom config files, the user can overwrite config.dev.toml's configurations. 

Config File
===========
The default preset ``config.dev.toml`` file can be located in ``codechain/config/presets/config.dev.toml``.

Config files can be customized by the user and its location can be designated by using the CLI command ``--config-path``. Custom config files created by the user must have the proper custom path.

Default config.dev.toml
=======================
The following represents the default configuration values of ``config.dev.toml``.
::

    [codechain]
    quiet = false
    db_path = "db"
    chain = "solo"
    enable_block_sync = true
    enable_parcel_relay = true
    secret_key = "0x0000000000000000000000000000000000000000000000000000000000000001"

CodeChain is set to use the Tendermint consensus algorithm by default. Tendermint is not suitable for solo testing purposes, since it requires a minimum of 4 users to function properly.

In order to test CodeChain alone, one may want to change ``change_type`` into Solo. To do this, use ``--chain solo``.

CLI Options for CodeChain client
================================
    ``--config-path=[PATH]``
        Specify the certain config file path that you want to use to configure CodeChain to your needs.

    ``--port=[PORT]``
        Listen for connections on PORT. (default: 3485)

    ``--bootstrap-addresses=[BOOTSTRAP_ADDRESSES]``
        Bootstrap addresses to connect.

    ``--no-network``
        Do not open network socket.

    ``--min-peers=[NUM]``
        Sets the minimum number of connections the user would like. (default: 10)

    ``--max-peers=[NUM]``
        Sets the maximum number of connections the user would like. (default: 30)

    ``--instance-id=[ID]``
        Specify instance id for logging. Used when running multiple instances of CodeChain.

    ``--quiet``
        Do not show any synchronization information in the console.

    ``--chain=[CHAIN]``
        Sets the blockchain type out of solo, solo_authority, tendermint or a path to chain spec file. (default: tendermint)

    ``--db-path=[PATH]``
        Specify the database directory path.

    ``--no-sync``
        Do not run block sync extension.

    ``--no-parcel-relay``
        Do not relay parcels.

    ``--jsonrpc-port=[PORT]``
        Listen for rpc connections on PORT. (default: 8080)

    ``--no-jsonrpc``
        Do not run jsonrpc.

    ``--secret-key=[KEY]``
        Secret key used by node.

    ``--author=[ADDRESS]``
        Specify the block's author (aka "coinbase") address for sending block rewards from 
        sealed blocks.

    ``--engine-signer=[ADDRESS]``
        Specify the address which should be used to sign consensus messages and 
        issue blocks.

    ``--no-discovery``
        Do not use discovery. No automated peer finding.

    ``--discovery="kademlia" | "unstructured"``
        Decides which p2p discovery extension to use. Options are `kademlia <https://github.com/CodeChain-io/codechain/wiki/Kademlia-Extension>`_ and unstructured.
        In a testing environment, an unstructured p2p network is desirable because it is
        more than sufficient when there are a few users.
        (default: unstructured)

    ``--discovery-bucket-size=[NUM]``
        Bucket size for discovery. Choose how many addresses to exchange at a time
        during discovery.

    ``--discovery-refresh=[ms]``
        Refresh timeout of discovery (ms). It may conflict with:`` --no-discovery``.

Logging
=======
For logging, run the following to configure:
``$ RUST_LOG=<level> codechain``

Log Levels
----------
CodeChain currently offers five different ``<level>``. They are error, warn, info, debug, and trace.

For example, the log level will be set to debug, if you run the following:

``$ RUST_LOG="debug" codechain``

* The **error** level represents an event where something can be dangerous, but can still run. In the case in which it cannot run anymore, it must crash ASAP instead of logging.

* The **warn** level represents an event which can be potentially dangerous.

* The **info** level represents an event which is not dangerous, but can be useful information for users.

* The **debug** level represents an event that is useful for developers, but not for users.

* The **trace** level is used for tracing.

Log Targets
-----------

Log levels can be set differently for each log targets. For example, you can run the following to set ``tx``'s log level as ``trace`` and ``parcel``'s 
log level as ``info`` with the following code:

``$ RUST_LOG="tx=trace,parcel=info" codechain``

The possible log targets are as follows:
::

    "blockchain"
    "client"
    "discovery"
    "engine"
    "external_parcel"
    "io"
    "mem_pool"
    "miner"
    "net"
    "netapi"
    "own_parcel"
    "poa"
    "shutdown"
    "snapshot"
    "solo_authoirty"
    "spec"
    "state"
    "state_db"
    "stratum"
    "sync"
    "test_script"
    "trie"
    "tx"