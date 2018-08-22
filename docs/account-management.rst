Account Management
##################

Subcommands
=============
CodeChain has ``account`` subcommand. It is used to manage accounts and has subcommands of its own, which are the following:

    ``create``
        Create a new account in the ``keys`` file directory. Upon creation, the user is asked to enter a passphrase.

    ``import <JSON_FILE_PATH>``
        Import a key in the format of a JSON file. Enter the directory that holds the JSON file to import.

    ``import-raw <RAW_KEY>``
        Import a private key(64 hexadecimal characters) directly.

    ``remove <ADDRESS>``
        Remove an account from the ``keys`` file directory. Use ``list`` to get the ADDRESS.

    ``list``
        List the managed accounts.

    ``change-password <ADDRESS>``
        Change the password of the account linked with the given ADDRESS.

Creating an Account
-------------------
You can create a new account with the ``create`` command. This command will ask for the user to create a password that goes along with the newly
created account.
::

    ./target/release/codechain account create

.. note::
    Password can be left blank by simply pressing the enter key twice after using the ``create`` command.

After creating an account with ``create``, you should see files created under ``/codechain/keys`` directory. These files should look something like this:
::

    UTC--2018-06-21T03-24-11Z--0995f73c-ddba-d65f-a6e5-083be0df4bbb

Upon closer inspection, the created accounts contain the following contents:
::

    {"id":"0995f73c-ddba-d65f-a6e5-083be0df4bbb","version":1,"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"e0b2af9a7f7676b547fae2c9e6b57694"},
    "ciphertext":"681389baba1ca30ba5b5610199168d819d00d318fef251279be0c5a48214c081","kdf":"pbkdf2","kdfparams":
    {"c":10240,"dklen":32,"prf":"hmac-sha256","salt":"ddce31fe0610f9d55e0ec1c28c04c11c02c5c19d3a5d64f910a43125a2922b04"},
    "mac":"7bc755edea0e64d8a1f14d9d38ebdfeabb791f8dad4f53175ed3c286e40610f7"},"address":"6753f53309a778291f96e339887c1644a8d596db","name":"","meta":"{}"}

Changing the Password
---------------------
You can change your password with the ``change-password`` command. For instance, if you want to change the password of cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7, run the following:
::

    ./target/release/codechain account change-password cccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9myd6c4d7

After entering the old password, a new password can be set. If the wrong password is entered, it will throw a KeystoreError.

Importing an Account
--------------------
Accounts can be imported in two ways. You can either define a certain directory or use a 64 character hexadecimal string. The first method can be done
by using the ``import`` command. Let's try importing a key from the ./keys directory. This can be done as follows:
::

     ./target/release/codechain account import ./keys/<NAME_OF_KEY>

The second method uses the ``import-raw`` command. Let's say you want to import a private key with the value of ``a159aa74f2dc23f560fdc36ad6f7ad597a8e61be4bb9e1a9edb50a9013574910``.
Then you would use the following command:
::

    ./target/release/codechain account import-raw a159aa74f2dc23f560fdc36ad6f7ad597a8e61be4bb9e1a9edb50a9013574910

The first method asks for the password of the key to import, since it is protected. The second method will ask you to set a new password for the 64 character hexadecimal string
of your choice.

Looking Up Accounts
-------------------
You can list all the accounts that are currently created by using the ``list`` command.

If you run the following, you should get a list of all the managed accounts' addresses:
::

    ./target/release/codechain account list

Removing Accounts
-----------------
If you want to remove a certain account, you should first know the address of that account. To do this, simply use the ``list`` command. Once you found the address of the
account you want to remove, simply use the ``remove`` command. If you want to delete an account with address ``0xc3bc9c4bd0020fcc9bd294c379b2eb7284c99de5``, then use the following command:
::

    ./target/release/codechain account remove 0xc3bc9c4bd0020fcc9bd294c379b2eb7284c99de5

Then you will be asked to enter the password. Once the correct password is entered, the account will be removed.
