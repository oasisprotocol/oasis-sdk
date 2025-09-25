# oasis-rofl-client

[![PyPI version](https://badge.fury.io/py/oasis-rofl-client.svg)](https://badge.fury.io/py/oasis-rofl-client)

Python client SDK for Oasis ROFL.

## Installation

```bash
pip install oasis-rofl-client
```

Or using [uv](https://docs.astral.sh/uv/):

```bash
uv add install oasis-rofl-client
```

The package requires Python 3.9+ and depends on `httpx` for async HTTP operations.

## Quickstart

The RoflClient provides async methods for interacting with ROFL services:

```python
import asyncio
from oasis_rofl_client import RoflClient

async def main():
    client = RoflClient()
    
    key = await client.generate_key("my-key-id")
    print(f"Generated SECP256K1 key: {key}")

asyncio.run(main())
```

## API Reference

### RoflClient

The main client class for interacting with ROFL runtime services.

#### Constructor

```python
RoflClient(url: str = '')
```

- `url`: Optional URL or Unix socket path
  - If empty (default): Uses Unix socket at `/run/rofl-appd.sock`
  - If starts with `http://` or `https://`: Uses HTTP transport
  - Otherwise: Treats as Unix socket path

#### Methods

##### `async generate_key(key_id: str, kind: KeyKind = KeyKind.SECP256K1) -> str`

Fetches or generates a cryptographic key from ROFL.

- **Parameters:**
  - `key_id`: Identifier for the key
  - `kind`: Type of key to generate (default: `KeyKind.SECP256K1`). Available options:
    - `KeyKind.RAW_256`: Generate 256 bits of entropy
    - `KeyKind.RAW_384`: Generate 384 bits of entropy
    - `KeyKind.ED25519`: Generate an Ed25519 private key
    - `KeyKind.SECP256K1`: Generate a Secp256k1 private key
- **Returns:** The private key as a hex string
- **Raises:** `httpx.HTTPStatusError` if the request fails

## Examples

For a complete working example, see [`examples/basic_usage.py`](examples/basic_usage.py).

## Release Process

Publishing to PyPI is fully automated via GitHub Actions.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE` for details or visit http://www.apache.org/licenses/LICENSE-2.0.