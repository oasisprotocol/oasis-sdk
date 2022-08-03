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

### ELF

Simply build the paratime in release mode using:

```bash
cargo build --release
```

The resulting binaries will be in `/src/target/release/`.

### Intel SGX

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

## Generating Bundles

Oasis Core since version 22.0 distributes bundles in the Oasis Runtime Container
format which is basically a zip archive with some metadata attached. This makes
it easier for node operators to configure paratimes. To ease creation of such
bundles from built binaries and metadata, you can use the `orc` tool provided by
the SDK.

:::info

You can install the `orc` utility by running:

```bash
go install github.com/oasisprotocol/oasis-sdk/tools/orc@latest
```

:::

The same bundle can contain both ELF and Intel SGX artifacts. To create a bundle
use the following command:

```bash
orc init path/to/elf-binary
```

When including Intel SGX artifacts you may additionally specify:

:::info

All bundles, even Intel SGX ones, are required to include an ELF binary of the
paratime. This binary is used for client nodes that don't have SGX support.

:::

```bash
orc init path/to/elf-binary --sgx-executable path/to/binary.sgxs --sgx-signature path/to/binary.sig
```

You can omit the signature initially and add it later by using:

```bash
orc sgx-set-sig bundle.orc path/to/binary.sig
```

### Multi-step SGX Signing Example

Multi-step signing allows enclave signing keys to be kept offline, preferrably
in some HSM. The following example uses `openssl` and a locally generated key as
an example, however, it is suggested that the key be stored in a more secure
location than in plaintext on disk.

#### Generate a key

We will generate a valid key for enclave signing. This must be a
3072-bit RSA key with a public exponent of 3. Do this like so:

```bash
openssl genrsa -3 3072 > private.pem
```

We will also need the public key in a later step so let's also generate this
now.

```bash
openssl rsa -in private.pem -pubout > public.pem
```

#### Generate signing data for your enclave

Generating signing data is done with the `orc sgx-gen-sign-data` subcommand,
like so:

```bash
orc sgx-gen-sign-data [options] bundle.orc
```

:::tip

See `orc sgx-gen-sign-data --help` for details on available options.

:::

For purposes of this example, let's assume your bundle is named `bundle.orc`.
You would generate data to sign like so:

```bash
orc sgx-gen-sign-data bundle.orc > sigstruct.sha256.bin
```

The output file `sigstruct.sha256.bin` contains the sha256 hash of the
SIGSTRUCT fields to be signed.

##### Sign the SIGSTRUCT hash

To sign the SIGSTRUCT you must create a signature using the `RSASSA-PKCS1-v1_5`
scheme. The following command will do so with `openssl`. If you're using an HSM,
your device may have a different process for generating a signature of this
type.

```bash
openssl pkeyutl -sign \
      -in sigstruct.sha256.bin \
      -inkey private.pem \
      -out sigstruct.sha256.sig \
      -pkeyopt digest:sha256
```

##### Attach the singed SIGSTRUCT to the bundle

With the signature in `sigstruct.sha256.sig` we can now generate a valid
SIGSTRUCT and attach it into the bundle.

```bash
orc sgx-set-sig bundle.orc sigstruct.sha256.sig public.pem
```

If there are no errors, `bundle.orc` will now contain a valid SGX SIGSTRUCT
that was signed by `private.pem`. To verify you can use `orc show` as follows.

```bash
orc show bundle.orc
```

It should return something like the following, showing the bundle content
including the signed SGX SIGSTRUCT (the signature is also verified):

```
Bundle:         /path/to/bundle.orc
Name:           my-paratime
Runtime ID:     000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c
Version:        0.1.1
Executable:     runtime.elf
SGXS:           runtime.sgx
SGXS MRENCLAVE: a68535bda1574a5e15dfb155c26e39bd404e9991a4d98010581a35d053011340
SGXS signature: runtime.sgx.sig
SGXS SIGSTRUCT:
  Build date:       2022-07-14 00:00:00 +0000 UTC
  MiscSelect:       00000000
  MiscSelect mask:  FFFFFFFF
  Attributes flags: 0000000000000004
    - 64-bit mode
  Attributes XFRM:  0000000000000003
  Attributes mask:  FFFFFFFFFFFFFFFD FFFFFFFFFFFFFFFC
  MRENCLAVE:        a68535bda1574a5e15dfb155c26e39bd404e9991a4d98010581a35d053011340
  ISV product ID:   0
  ISV SVN:          0
Digests:
  runtime.sgx.sig => 3c0daea89dfdb3d0381147dec3e041a596617f686afa9b28436ca17980dafee4
  runtime.elf => a96397fc309bc2116802315c0341a2a9f6f21935d79a3f56d71b3e4d6f6d9302
  runtime.sgx => b96ff3ae9c73646459b7e8dc1d096838720a7c62707affc1800967cbee99b28b
```
