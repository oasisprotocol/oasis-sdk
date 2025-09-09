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

    async def _appd_post(self, path: str, payload: Any) -> Any:
        """Post request to ROFL application daemon.

        Args:
            path: API endpoint path
            payload: JSON payload to send

        Returns:
            JSON response from the daemon

        Raises:
            httpx.HTTPStatusError: If the request fails
        """
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
            logger.debug(f"Posting to {full_url}: {json.dumps(payload)}")
            response: httpx.Response = await client.post(
                full_url, json=payload, timeout=60.0
            )
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
        response: dict[str, Any] = await self._appd_post(path, payload)
        return response["key"]
