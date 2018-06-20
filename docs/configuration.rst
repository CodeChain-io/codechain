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
    chain_type = "tendermint"
    enable_block_sync = true
    enable_parcel_relay = true
    secret_key = "0x0000000000000000000000000000000000000000000000000000000000000001"

CodeChain is set to use the Tendermint consensus algorithm by default. Tendermint is not suitable for solo testing purposes, since it requires a minimum of 4 users to function properly.

In order to test CodeChain alone, one may want to change ``change_type`` into Solo. To do this, use ``--chain solo``.

CLI Options for CodeChain client
================================
::

    --config-path
        Specify the certain config file path that you want to use to configure CodeChain to your needs.

    --port
        Listen for connections on PORT. (default: 3485)

    --bootstrap-addresses
        Bootstrap addresses to connect.

    --no-network
        Do not open network socket.

    --min-peers
        Sets the minimum number of connections the user would like. (default: 10)

    --max-peers
        Sets the maximum number of connections the user would like. (default: 30)

    --instance-id
        Specify instance id for logging.

    --quiet
        Do not show any synchronization information in the console.

    --chain
        Sets the blockchain type out of solo, solo_authority, tendermint or a path to chain spec file. (default: tendermint)

    --db-path
        Specify the database directory path.

    --no-sync
        Do not run block sync extension.

    --no-parcel-relay
        Do not relay parcels.

    --jsonrpc-port
        Listen for rpc connections on PORT. (default: 8080)

    --no-jsonrpc
        Do not run jsonrpc.

    --secret-key
        Secret key used by node.

    --author
        Specify the block author (aka "coinbase") address for sending block rewards from 
        sealed blocks.

    --engine-signer
        Specify the address which should be used to sign consensus messages and 
        issue blocks.

    --no-discovery
        Do not use discovery.

    --discovery
        p2p discovery extension. Options are kademlia and unstructured. (default: unstructured)

    --kademlia-alpha
        Degree of parallelism in Kademlia.

    --discovery-bucket-size
        Bucket size for discovery.

    --discovery-refresh
        Refresh timeout of discovery (ms). (Conflicts with: --no-discovery)

    Subcommands

    CodeChain has a subcommand called ``account``. ``amount``is the account managing commands of CodeChain, and also has subcommands of its own, which are the following:

        Subcommands of codechain account:
            --create
                about: create account
                args:
                    - passphrase:
                        short: p
                        long: passphrase
                        help: account passphrase
                        takes_value: true
            --import
                about: import private key
                args:
                    - passphrase:
                        short: p
                        long: passphrase
                        help: account passphrase
                        takes_value: true
                    - raw-key:
                        short: k
                        long: raw-key
                        help: key to import
                        takes_value: true
            --list
                about: list managed accounts
            --lock
                about: lock account
                args:
                    - address:
                        help: address to lock
            --unlock
                about: unlock account
                args:
                    - address:
                        help: address to unlock