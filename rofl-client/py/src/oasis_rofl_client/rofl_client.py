"""ROFL client for interacting with ROFL REST API.

Provides methods for key generation through the ROFL REST API.
"""

from __future__ import annotations

import json
import logging
from enum import Enum
from typing import Any

import httpx

logger = logging.getLogger(__name__)


class KeyKind(Enum):
    """Supported key generation types for ROFL.

    Attributes:
        RAW_256: Generate 256 bits of entropy
        RAW_384: Generate 384 bits of entropy
        ED25519: Generate an Ed25519 private key
        SECP256K1: Generate a Secp256k1 private key
    """

    RAW_256 = "raw-256"
    RAW_384 = "raw-384"
    ED25519 = "ed25519"
    SECP256K1 = "secp256k1"


class RoflClient:
    """Client for interacting with ROFL REST API.

    Provides methods for key fetching through the ROFL REST API.
    """

    ROFL_SOCKET_PATH: str = "/run/rofl-appd.sock"

    def __init__(self, url: str = "") -> None:
        """Initialize ROFL client.

        Args:
            url: Optional URL for HTTP transport (defaults to Unix socket)
        """
        self.url: str = url

    async def _appd_request(
        self, method: str, path: str, payload: Any = None
    ) -> Any:
        """Request to ROFL application daemon.

        Args:
            method: HTTP method (GET or POST)
            path: API endpoint path
            payload: JSON payload to send (for POST requests)

        Returns:
            JSON response from the daemon

        Raises:
            ValueError: If an unsupported HTTP method is provided
            httpx.HTTPStatusError: If the request fails
        """
        if method not in ("GET", "POST"):
            raise ValueError(f"Unsupported HTTP method: {method}")

        transport: httpx.AsyncHTTPTransport | None = None

        if self.url and not self.url.startswith("http"):
            transport = httpx.AsyncHTTPTransport(uds=self.url)
            logger.debug(f"Using HTTP socket: {self.url}")
        elif not self.url:
            transport = httpx.AsyncHTTPTransport(uds=self.ROFL_SOCKET_PATH)
            logger.debug(f"Using unix domain socket: {self.ROFL_SOCKET_PATH}")

        async with httpx.AsyncClient(transport=transport) as client:
            base_url: str = (
                self.url
                if self.url and self.url.startswith("http")
                else "http://localhost"
            )
            full_url: str = base_url + path

            if method == "GET":
                logger.debug(f"Getting from {full_url}")
                response: httpx.Response = await client.get(full_url, timeout=60.0)
            else:  # POST
                logger.debug(f"Posting to {full_url}: {json.dumps(payload)}")
                response = await client.post(full_url, json=payload, timeout=60.0)

            response.raise_for_status()
            return response.json()

    async def generate_key(
        self, key_id: str, kind: KeyKind = KeyKind.SECP256K1
    ) -> str:
        """Fetch or generate a cryptographic key from ROFL.

        Args:
            key_id: Identifier for the key
            kind: Type of key to generate (default: SECP256K1)

        Returns:
            The private key as a hex string

        Raises:
            httpx.HTTPStatusError: If key fetch fails
        """
        payload: dict[str, str] = {
            "key_id": key_id,
            "kind": kind.value,
        }

        path: str = "/rofl/v1/keys/generate"
        response: dict[str, Any] = await self._appd_request("POST", path, payload)
        return response["key"]

    async def get_metadata(self) -> dict[str, str]:
        """Get all user-set metadata key-value pairs.

        Returns:
            Dictionary of metadata key-value pairs

        Raises:
            httpx.HTTPStatusError: If the request fails
        """
        path: str = "/rofl/v1/metadata"
        response: dict[str, str] = await self._appd_request("GET", path)
        return response

    async def set_metadata(self, metadata: dict[str, str]) -> None:
        """Set metadata key-value pairs.

        This replaces all existing app-provided metadata. Will trigger a registration
        refresh if the metadata has changed.

        Args:
            metadata: Dictionary of metadata key-value pairs to set

        Raises:
            httpx.HTTPStatusError: If the request fails
        """
        path: str = "/rofl/v1/metadata"
        await self._appd_request("POST", path, metadata)

    async def query(self, method: str, args: bytes) -> bytes:
        """Query the on-chain paratime state.

        Args:
            method: The query method name
            args: CBOR-encoded query arguments

        Returns:
            CBOR-encoded response data

        Raises:
            httpx.HTTPStatusError: If the request fails
        """
        payload: dict[str, str] = {
            "method": method,
            "args": args.hex(),
        }

        path: str = "/rofl/v1/query"
        response: dict[str, str] = await self._appd_request("POST", path, payload)
        return bytes.fromhex(response["data"])
