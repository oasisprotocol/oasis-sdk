"""Tests for RoflClient."""

import unittest
from unittest.mock import AsyncMock, MagicMock, patch

from oasis_rofl_client import KeyKind, RoflClient


class TestRoflClient(unittest.IsolatedAsyncioTestCase):
    """Test cases for RoflClient."""

    def test_init_default(self):
        """Test client initialization with default settings."""
        client = RoflClient()
        self.assertEqual(client.url, "")
        self.assertEqual(client.ROFL_SOCKET_PATH, "/run/rofl-appd.sock")

    def test_init_with_url(self):
        """Test client initialization with custom URL."""
        client = RoflClient(url="https://example.rofl")
        self.assertEqual(client.url, "https://example.rofl")

    def test_init_with_socket_path(self):
        """Test client initialization with custom socket path."""
        client = RoflClient(url="/custom/socket.sock")
        self.assertEqual(client.url, "/custom/socket.sock")

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_generate_key(self, mock_client_class):
        """Test generate_key method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0x123456789abcdef"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test key generation
        client = RoflClient()
        key = await client.generate_key("test-key-id")

        # Verify the result
        self.assertEqual(key, "0x123456789abcdef")

        # Verify the API call
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "test-key-id", "kind": "secp256k1"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_generate_key_with_http_url(self, mock_client_class):
        """Test generate_key method with HTTP URL."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0xfedcba987654321"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test with HTTP URL
        client = RoflClient(url="https://rofl.example.com")
        key = await client.generate_key("another-key")

        # Verify the result
        self.assertEqual(key, "0xfedcba987654321")

        # Verify the API call uses the custom URL
        mock_client.post.assert_called_once_with(
            "https://rofl.example.com/rofl/v1/keys/generate",
            json={"key_id": "another-key", "kind": "secp256k1"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncHTTPTransport")
    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_unix_socket_transport(
        self, mock_client_class, mock_transport_class
    ):
        """Test that Unix socket transport is used correctly."""
        # Setup mocks
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0xabcdef"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        mock_transport = MagicMock()
        mock_transport_class.return_value = mock_transport

        # Test with default Unix socket
        client = RoflClient()
        await client.generate_key("socket-key")

        # Verify Unix socket transport was created
        mock_transport_class.assert_called_once_with(uds="/run/rofl-appd.sock")
        mock_client_class.assert_called_once_with(transport=mock_transport)

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncHTTPTransport")
    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_custom_socket_transport(
        self, mock_client_class, mock_transport_class
    ):
        """Test that custom socket path is used correctly."""
        # Setup mocks
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0x123"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        mock_transport = MagicMock()
        mock_transport_class.return_value = mock_transport

        # Test with custom socket path
        client = RoflClient(url="/custom/path.sock")
        await client.generate_key("custom-key")

        # Verify custom socket transport was created
        mock_transport_class.assert_called_once_with(uds="/custom/path.sock")
        mock_client_class.assert_called_once_with(transport=mock_transport)

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_generate_key_with_ed25519(self, mock_client_class):
        """Test generate_key with Ed25519 key kind."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0xed25519key"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test key generation with Ed25519
        client = RoflClient()
        key = await client.generate_key("ed25519-key", kind=KeyKind.ED25519)

        # Verify the result
        self.assertEqual(key, "0xed25519key")

        # Verify the API call uses ed25519
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "ed25519-key", "kind": "ed25519"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_generate_key_with_raw_256(self, mock_client_class):
        """Test generate_key with RAW_256 entropy."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0xraw256entropy"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test key generation with RAW_256
        client = RoflClient()
        key = await client.generate_key("entropy-256", kind=KeyKind.RAW_256)

        # Verify the result
        self.assertEqual(key, "0xraw256entropy")

        # Verify the API call uses raw-256
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "entropy-256", "kind": "raw-256"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.rofl_client.httpx.AsyncClient")
    async def test_generate_key_with_raw_384(self, mock_client_class):
        """Test generate_key with RAW_384 entropy."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key": "0xraw384entropy"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test key generation with RAW_384
        client = RoflClient()
        key = await client.generate_key("entropy-384", kind=KeyKind.RAW_384)

        # Verify the result
        self.assertEqual(key, "0xraw384entropy")

        # Verify the API call uses raw-384
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "entropy-384", "kind": "raw-384"},
            timeout=60.0,
        )

    def test_key_kind_enum_values(self):
        """Test that KeyKind enum has correct values."""
        self.assertEqual(KeyKind.RAW_256.value, "raw-256")
        self.assertEqual(KeyKind.RAW_384.value, "raw-384")
        self.assertEqual(KeyKind.ED25519.value, "ed25519")
        self.assertEqual(KeyKind.SECP256K1.value, "secp256k1")


if __name__ == "__main__":
    unittest.main()
