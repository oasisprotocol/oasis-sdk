"""ROFL client for interacting with ROFL REST API.

Provides methods for key generation through the ROFL REST API.
"""

from __future__ import annotations

import json
import logging
from enum import Enum
from typing import Any

import cbor2
import httpx
from web3.types import TxParams

logger = logging.getLogger(__name__)


class KeyKind(Enum):
    """Supported key generation types for ROFL.

    :cvar RAW_256: Generate 256 bits of entropy
    :cvar RAW_384: Generate 384 bits of entropy
    :cvar ED25519: Generate an Ed25519 private key
    :cvar SECP256K1: Generate a Secp256k1 private key
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

        :param url: Optional URL for HTTP transport (defaults to Unix socket)
        """
        self.url: str = url

    async def _appd_request(
        self, method: str, path: str, payload: Any = None
    ) -> httpx.Response:
        """Request to ROFL application daemon.

        :param method: HTTP method (GET or POST)
        :param path: API endpoint path
        :param payload: JSON payload to send (for POST requests)
        :returns: HTTP response from the daemon
        :raises ValueError: If an unsupported HTTP method is provided
        :raises httpx.HTTPStatusError: If the request fails
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
                response: httpx.Response = await client.get(
                    full_url, timeout=60.0
                )
            else:  # POST
                logger.debug(f"Posting to {full_url}: {json.dumps(payload)}")
                response = await client.post(
                    full_url, json=payload, timeout=60.0
                )

            response.raise_for_status()

            if not response.content:
                return None

            return response

    async def get_app_id(self) -> str:
        """Retrieve the app ID.

        :returns: The app ID as Bech32-encoded string
        :raises httpx.HTTPStatusError: If key fetch fails
        """
        path: str = "/rofl/v1/app/id"
        return (await self._appd_request("GET", path)).text

    async def generate_key(
        self, key_id: str, kind: KeyKind = KeyKind.SECP256K1
    ) -> str:
        """Fetch or generate a cryptographic key from ROFL.

        :param key_id: Identifier for the key
        :param kind: Type of key to generate (default: SECP256K1)
        :returns: The private key as a hex string
        :raises httpx.HTTPStatusError: If key fetch fails
        """
        payload: dict[str, str] = {
            "key_id": key_id,
            "kind": kind.value,
        }

        path: str = "/rofl/v1/keys/generate"
        response: dict[str, Any] = (
            await self._appd_request("POST", path, payload)
        ).json()
        return response["key"]

    async def sign_submit(
        self,
        tx: TxParams,
        encrypt: bool = True,
    ) -> dict[str, Any]:
        """Sign the given Ethereum transaction with an endorsed ephemeral key and submit it to Sapphire.

        Note: Transaction nonce and gas price are ignored.

        :param tx: Transaction parameters
        :param encrypt: End-to-end encrypt the transaction before submitting (default: True)
        :returns: Deserialized response data object.
        :raises httpx.HTTPStatusError: If the request fails
        :raises cbor2.CBORDecodeValueError: If the response data is invalid
        """
        payload = {
            "tx": {
                "kind": "eth",
                "data": {
                    "gas_limit": tx["gas"],
                    "value": str(tx["value"]),
                    "data": tx["data"][2:]
                    if tx["data"].startswith("0x")
                    else tx["data"],
                },
            },
            "encrypt": encrypt,
        }

        # Contract create transactions don't have "to". For others, include it.
        if "to" in tx:
            payload["tx"]["data"]["to"] = (
                tx["to"][2:] if tx["to"].startswith("0x") else tx["to"]
            )

        path = "/rofl/v1/tx/sign-submit"

        response: dict[str, str] = (
            await self._appd_request("POST", path, payload)
        ).json()
        result = {}
        # Decode CBOR-encoded data field to python object.
        if response.get("data"):
            result = cbor2.loads(bytes.fromhex(response["data"]))
        return result

    async def get_metadata(self) -> dict[str, str]:
        """Get all user-set metadata key-value pairs.

        :returns: Dictionary of metadata key-value pairs
        :raises httpx.HTTPStatusError: If the request fails
        """
        path: str = "/rofl/v1/metadata"
        response: dict[str, str] = (
            await self._appd_request("GET", path)
        ).json()
        return response

    async def set_metadata(self, metadata: dict[str, str]) -> None:
        """Set metadata key-value pairs.

        This replaces all existing app-provided metadata. Will trigger a registration
        refresh if the metadata has changed.

        :param metadata: Dictionary of metadata key-value pairs to set
        :raises httpx.HTTPStatusError: If the request fails
        """
        path: str = "/rofl/v1/metadata"
        await self._appd_request("POST", path, metadata)

    async def query(self, method: str, args: bytes) -> bytes:
        """Query the on-chain paratime state.

        :param method: The query method name
        :param args: CBOR-encoded query arguments
        :returns: CBOR-encoded response data
        :raises httpx.HTTPStatusError: If the request fails
        """
        payload: dict[str, str] = {
            "method": method,
            "args": args.hex(),
        }

        path: str = "/rofl/v1/query"
        response: dict[str, str] = (
            await self._appd_request("POST", path, payload)
        ).json()
        return bytes.fromhex(response["data"])
