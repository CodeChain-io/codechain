Setup
#####

Build Dependencies
==================

CodeChain requires Rust version 1.26 to build. Using `rustup <https://rustup.rs/>`_ is recommended.

* For Linux Systems:

    * Ubuntu
    .. note::
        ``gcc`` and ``g++`` are required for installing packages.
::

    $ curl https://sh.rustup.rs -sSf | sh

* For Mac Systems:

    * MacOS 10.13.2 (17C88) tested
    .. note::
        ``clang`` is required for installing packages.
::

    $ curl https://sh.rustup.rs -sSf | sh

* For Windows Systems:

    * Currently not supported for Windows. If on a Windows system, please install `WSL <https://docs.microsoft.com/en-us/windows/wsl/install-win10>`_ to continue as Ubuntu.

Please make sure that all of the binaries above are included in your ``PATH``. These conditions must be fulfilled before building CodeChain from source.

Building From Source
====================

Download CodeChain's source code and go into its directory.
::

    git clone git@github.com:CodeChain-io/codechain.git
    cd codechain


Build as Release Version
------------------------
::

    cargo build --release

This will produce an executable in the ./target/release directory.
