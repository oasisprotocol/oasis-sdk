#!/usr/bin/env python
"""Basic usage example for oasis-rofl-client."""

import asyncio
import logging
import sys
from pathlib import Path

# Add parent directory to path for development
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from oasis_rofl_client import KeyKind, RoflClient

# Configure logging to see debug messages
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)


async def main():
    """Demonstrate basic RoflClient usage."""

    # Create client with default Unix socket
    client = RoflClient()
    print(f"Client created with default socket: {client.ROFL_SOCKET_PATH}")

    # Generate a key with default type (SECP256K1)
    try:
        key = await client.generate_key("my-first-key")
        print(f"Generated SECP256K1 key: {key}")

        # Generate an Ed25519 key
        ed_key = await client.generate_key(
            "my-ed25519-key", kind=KeyKind.ED25519
        )
        print(f"Generated Ed25519 key: {ed_key}")

        # Generate raw entropy (256 bits)
        entropy = await client.generate_key("my-entropy", kind=KeyKind.RAW_256)
        print(f"Generated 256-bit entropy: {entropy}")

    except Exception as e:
        print("Note: Key generation requires a running ROFL service")
        print(f"Error: {e}")


if __name__ == "__main__":
    asyncio.run(main())
