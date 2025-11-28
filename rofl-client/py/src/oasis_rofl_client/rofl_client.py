"""ROFL client for interacting with ROFL REST API.

Provides methods for key generation through the ROFL REST API.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any

from web3.types import TxParams

from .async_rofl_client import AsyncRoflClient, KeyKind

logger = logging.getLogger(__name__)


class RoflClient:
    """Client for interacting with ROFL REST API.

    Provides methods for key fetching through the ROFL REST API.
    """

    def __init__(self, url: str = "") -> None:
        """Initialize ROFL client.

        :param url: Optional URL for HTTP transport (defaults to Unix socket)
        """
        self.client = AsyncRoflClient(url)

    def get_app_id(self) -> str:
        """Retrieve the app ID.

        :returns: The app ID as Bech32-encoded string
        :raises httpx.HTTPStatusError: If key fetch fails
        """
        return asyncio.run(self.client.get_app_id())

    def generate_key(
        self, key_id: str, kind: KeyKind = KeyKind.SECP256K1
    ) -> str:
        """Fetch or generate a cryptographic key from ROFL.

        :param key_id: Identifier for the key
        :param kind: Type of key to generate (default: SECP256K1)
        :returns: The private key as a hex string
        :raises httpx.HTTPStatusError: If key fetch fails
        """

        return asyncio.run(self.client.generate_key(key_id, kind))

    def sign_submit(
        self,
        tx: TxParams,
        encrypt: bool = False,
    ) -> dict[str, Any]:
        """Sign the given Ethereum transaction with an endorsed ephemeral key and submit it to Sapphire.

        Note: Transaction nonce and gas price are ignored.

        :param tx: Transaction parameters
        :param encrypt: End-to-end encrypt the transaction before submitting (default: False)
        :returns: Deserialized response data object.
        :raises httpx.HTTPStatusError: If the request fails
        :raises cbor2.CBORDecodeValueError: If the response data is invalid
        """
        return asyncio.run(self.client.sign_submit(tx, encrypt))

    def get_metadata(self) -> dict[str, str]:
        """Get all user-set metadata key-value pairs.

        :returns: Dictionary of metadata key-value pairs
        :raises httpx.HTTPStatusError: If the request fails
        """
        return asyncio.run(self.client.get_metadata())

    def set_metadata(self, metadata: dict[str, str]) -> None:
        """Set metadata key-value pairs.

        This replaces all existing app-provided metadata. Will trigger a registration
        refresh if the metadata has changed.

        :param metadata: Dictionary of metadata key-value pairs to set
        :raises httpx.HTTPStatusError: If the request fails
        """
        asyncio.run(self.client.set_metadata(metadata))

    def query(self, method: str, args: bytes) -> bytes:
        """Query the on-chain paratime state.

        :param method: The query method name
        :param args: CBOR-encoded query arguments
        :returns: CBOR-encoded response data
        :raises httpx.HTTPStatusError: If the request fails
        """
        return asyncio.run(self.client.query(method, args))
