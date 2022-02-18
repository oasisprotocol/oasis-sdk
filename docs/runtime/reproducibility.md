# Reproducibility

If you wish to build paratime binaries yourself, you can use the
environment provided as part of the SDK. This way you can also verify
that the binaries match the ones running on the network.

The steps below show how to build the test runtimes provided in the
`oasis-sdk` sources; steps for other paratimes should be similar.

## Environment Setup

The build environment is provided as a Docker image containing all the
necessary tools. Refer to your system's documentation for pointers on
installing software.

The runtime sources need to be mounted into the container so prepare a
directory first, such as:

```bash
git clone https://github.com/oasisprotocol/oasis-sdk.git
```

## Running the Image

The images are available in the `oasisprotocol/runtime-builder`
repository on Docker Hub and are tagged with the same version numbers as
releases of the SDK. To pull the image and run a container with it, run
the following:

```bash
docker run -t -i -v /home/user/oasis-sdk:/src oasisprotocol/runtime-builder:main /bin/bash
```

where:

- `/home/user/oasis-sdk` is the absolute path to the directory
  containing the SDK sources (or other paratimes - you likely do not need
  to download the SDK separately if you're building other paratimes), and
- `main` is a release of the SDK - the documentation of the paratime
  you're trying to build should mention the version required.

This gives you a root shell in the container. Rust and Cargo are
installed in `/cargo`, Go in `/go`, and the sources to your paratime are
available in `/src`.

## Building

Follow the normal build procedure for your paratime. For the testing
runtimes in the SDK, e.g.:

```bash
cd /src
cargo build --release --target x86_64-fortanix-unknown-sgx
```

After this step is complete, the binaries will be in
`/src/target/x86_64-fortanix-unknown-sgx/release/`.

To produce the sgxs format needed on the Oasis network, change directory
to where a particular runtime's `Cargo.toml` file is and run the
following command:

```bash
cargo elf2sgxs --release
```

It is necessary to change directories first because the tool does not
currently support cargo workspaces.

The resulting binaries will have the `.sgxs` extension.
