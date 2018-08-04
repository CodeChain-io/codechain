.. _setup:

Setup
#####

Using Docker
===================
CodeChain supports the use of Docker to provide an easy and seamless installation process by providing a single package that gives the user everything he/she
needs to get CodeChain up and running. In order to get the installation package, run the following command after installing Docker:
::

    docker build -f docker/ubuntu/Dockerfile --tag codechain-io/codechain:branch_or_tag_name .

WSL users may find difficulty in using Docker, and thus, it is highly recommended to use Ubuntu, or install Docker for Windows. When using Docker for Windows,
it is necessary to enable Hyper-V in BIOS settings.

To see the Docker images created, run the following:
::

    docker images

It will result in something like this:
::

    REPOSITORY               TAG                  IMAGE ID            CREATED              SIZE
    codechain-io/codechain   branch_or_tag_name   6f8474d9bc7a        About a minute ago   1.85GB
    ubuntu                   14.04                971bb384a50a        6 days ago           188MB

If you want to run the first image file, run the following command:
::

    docker run -it codechain-io/codechain:branch_or_tag_name

This should result in CodeChain running.

Build Dependencies
==================

CodeChain requires Rust version 1.26 to build. Using `rustup <https://rustup.rs/>`_ is recommended.

* For Linux Systems:

    * Ubuntu

    .. note::
        ``gcc``, ``g++`` and ``make`` are required for installing packages.

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

Using CodeChain SDK
=========================
Before starting to use the CodeChain SDK, please install node.js by going to this `page <https://nodejs.org/en/>`_.

Next, install the package with the following command:

``npm install codechain-sdk`` or ``yarn add codechain-sdk``