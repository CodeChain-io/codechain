Account Management
##################

Subcommands
=============
CodeChain has a subcommand called ``account``. These commands are used to manage accounts. It has subcommands of its own, which are the following:

    ``create``
        Creates a new account in the ``keys`` file directory. Upon creation, the user is asked to enter a passphrase.

    ``import <JSON_FILE_PATH>``
        Imports a key in the format of a JSON file. Enter the directory that holds the JSON file to import.

    ``import-raw <RAW_KEY>``
        Imports a private key(64 hexadecimal characters) directly.

    ``remove <ADDRESS>``
        Removes an account from the ``keys`` file directory. Use ``list`` to get the ADDRESS.

    ``list``
        List the managed accounts.

Creating an Account
-------------------
You can create a new account with the ``create`` command. You can add a password as well. For example, if you want to create an account with a password of '1234',
run the following:
::

    ./target/release/codechain account create --passphrase 1234

After creating an account with ``create``, you should see files created under ``/codechain/keystoreData`` directory. These files should look something like this:
::

    UTC--2018-06-21T03-24-11Z--0995f73c-ddba-d65f-a6e5-083be0df4bbb

Upon closer inspection, the created accounts contain the following contents:
::

    {"id":"0995f73c-ddba-d65f-a6e5-083be0df4bbb","version":1,"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"e0b2af9a7f7676b547fae2c9e6b57694"},
    "ciphertext":"681389baba1ca30ba5b5610199168d819d00d318fef251279be0c5a48214c081","kdf":"pbkdf2","kdfparams":
    {"c":10240,"dklen":32,"prf":"hmac-sha256","salt":"ddce31fe0610f9d55e0ec1c28c04c11c02c5c19d3a5d64f910a43125a2922b04"},
    "mac":"7bc755edea0e64d8a1f14d9d38ebdfeabb791f8dad4f53175ed3c286e40610f7"},"address":"6753f53309a778291f96e339887c1644a8d596db","name":"","meta":"{}"}

Looking Up Accounts
-------------------
You can list all the accounts that are currently created by using the ``list`` command.

If you run the following, you should get a list of all the managed accounts' addresses:
::

    ./target/release/codechain account list
