"""Tests for AsyncRoflClient."""

import unittest
from unittest.mock import AsyncMock, MagicMock, patch

from web3.types import TxParams

from oasis_rofl_client import AsyncRoflClient, KeyKind
from oasis_rofl_client.common import ROFL_SOCKET_PATH


class TestAsyncRoflClient(unittest.IsolatedAsyncioTestCase):
    """Test cases for RoflClient."""

    def test_init_default(self):
        """Test client initialization with default settings."""
        client = AsyncRoflClient()
        self.assertEqual(client.url, "")
        self.assertEqual(ROFL_SOCKET_PATH, "/run/rofl-appd.sock")

    def test_init_with_url(self):
        """Test client initialization with custom URL."""
        client = AsyncRoflClient(url="https://example.rofl")
        self.assertEqual(client.url, "https://example.rofl")

    def test_init_with_socket_path(self):
        """Test client initialization with custom socket path."""
        client = AsyncRoflClient(url="/custom/socket.sock")
        self.assertEqual(client.url, "/custom/socket.sock")

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient()
        key = await client.generate_key("test-key-id")

        # Verify the result
        self.assertEqual(key, "0x123456789abcdef")

        # Verify the API call
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "test-key-id", "kind": "secp256k1"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient(url="https://rofl.example.com")
        key = await client.generate_key("another-key")

        # Verify the result
        self.assertEqual(key, "0xfedcba987654321")

        # Verify the API call uses the custom URL
        mock_client.post.assert_called_once_with(
            "https://rofl.example.com/rofl/v1/keys/generate",
            json={"key_id": "another-key", "kind": "secp256k1"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncHTTPTransport")
    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient()
        await client.generate_key("socket-key")

        # Verify Unix socket transport was created
        mock_transport_class.assert_called_once_with(uds="/run/rofl-appd.sock")
        mock_client_class.assert_called_once_with(transport=mock_transport)

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncHTTPTransport")
    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient(url="/custom/path.sock")
        await client.generate_key("custom-key")

        # Verify custom socket transport was created
        mock_transport_class.assert_called_once_with(uds="/custom/path.sock")
        mock_client_class.assert_called_once_with(transport=mock_transport)

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient()
        key = await client.generate_key("ed25519-key", kind=KeyKind.ED25519)

        # Verify the result
        self.assertEqual(key, "0xed25519key")

        # Verify the API call uses ed25519
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "ed25519-key", "kind": "ed25519"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient()
        key = await client.generate_key("entropy-256", kind=KeyKind.RAW_256)

        # Verify the result
        self.assertEqual(key, "0xraw256entropy")

        # Verify the API call uses raw-256
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/keys/generate",
            json={"key_id": "entropy-256", "kind": "raw-256"},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
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
        client = AsyncRoflClient()
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

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_get_app_id(self, mock_client_class):
        """Test get_app_id method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.text = "rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf"
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.get.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test get metadata
        client = AsyncRoflClient()
        app_id = await client.get_app_id()

        # Verify the result
        self.assertEqual(
            app_id, "rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf"
        )

        # Verify the API call
        mock_client.get.assert_called_once_with(
            "http://localhost/rofl/v1/app/id", timeout=60.0
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_sign_submit(self, mock_client_class):
        """Test sign_submit method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {
            "data": "a1646661696ca364636f646508666d6f64756c656365766d676d6573736167657272657665727465643a20614a416f4c773d3d"
        }
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test set metadata
        client = AsyncRoflClient()
        tx: TxParams = {
            "from": "0x1234567890123456789012345678901234567890",
            "to": "0x0987654321098765432109876543210987654321",
            "data": "0xdae1ee1f00000000000000000000000000000000000000000000000000002695a9e649b2",
            "gas": 21000,
            "gasPrice": 1000000000,
            "value": 1000000000,
            "nonce": 0,
        }

        response = await client.sign_submit(tx, True)

        self.assertEqual(
            response,
            {
                "fail": {
                    "code": 8,
                    "module": "evm",
                    "message": "reverted: aJAoLw==",
                }
            },
        )

        # Verify the API call
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/tx/sign-submit",
            json={
                "tx": {
                    "kind": "eth",
                    "data": {
                        "gas_limit": 21000,
                        "value": "1000000000",
                        "data": "dae1ee1f00000000000000000000000000000000000000000000000000002695a9e649b2",
                        "to": "0987654321098765432109876543210987654321",
                    },
                },
                "encrypt": True,
            },
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_sign_submit_default_encrypt_true(self, mock_client_class):
        """sign_submit should default encrypt to True when omitted."""
        mock_response = MagicMock()
        mock_response.json.return_value = {"data": ""}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        client = AsyncRoflClient()
        tx: TxParams = {
            "from": "0x1234567890123456789012345678901234567890",
            "data": "0x",
            "gas": 21000,
            "gasPrice": 0,
            "value": 0,
            "nonce": 0,
        }

        await client.sign_submit(tx)  # no encrypt argument

        mock_client.post.assert_called_once()
        _, kwargs = mock_client.post.call_args
        assert kwargs["json"]["encrypt"] is True

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_get_metadata(self, mock_client_class):
        """Test get_metadata method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"key1": "value1", "key2": "value2"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.get.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test get metadata
        client = AsyncRoflClient()
        metadata = await client.get_metadata()

        # Verify the result
        self.assertEqual(metadata, {"key1": "value1", "key2": "value2"})

        # Verify the API call
        mock_client.get.assert_called_once_with(
            "http://localhost/rofl/v1/metadata", timeout=60.0
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_set_metadata(self, mock_client_class):
        """Test set_metadata method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.content = b""
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test set metadata
        client = AsyncRoflClient()
        metadata_to_set = {
            "new_key": "new_value",
            "another_key": "another_value",
        }
        await client.set_metadata(metadata_to_set)

        # Verify the API call
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/metadata",
            json=metadata_to_set,
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_get_metadata_with_http_url(self, mock_client_class):
        """Test get_metadata method with HTTP URL."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"custom": "metadata"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.get.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test with HTTP URL
        client = AsyncRoflClient(url="https://rofl.example.com")
        metadata = await client.get_metadata()

        # Verify the result
        self.assertEqual(metadata, {"custom": "metadata"})

        # Verify the API call uses the custom URL
        mock_client.get.assert_called_once_with(
            "https://rofl.example.com/rofl/v1/metadata", timeout=60.0
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_query(self, mock_client_class):
        """Test query method."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {
            "data": "48656c6c6f"
        }  # "Hello" in hex
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test query
        client = AsyncRoflClient()
        args = b"\xa1\x64test\x65value"  # CBOR-encoded test data
        result = await client.query("test.Method", args)

        # Verify the result
        self.assertEqual(result, b"Hello")

        # Verify the API call
        mock_client.post.assert_called_once_with(
            "http://localhost/rofl/v1/query",
            json={"method": "test.Method", "args": args.hex()},
            timeout=60.0,
        )

    @patch("oasis_rofl_client.async_rofl_client.httpx.AsyncClient")
    async def test_query_with_http_url(self, mock_client_class):
        """Test query method with HTTP URL."""
        # Setup mock
        mock_response = MagicMock()
        mock_response.json.return_value = {"data": "deadbeef"}
        mock_response.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value.__aenter__.return_value = mock_client

        # Test with HTTP URL
        client = AsyncRoflClient(url="https://rofl.example.com")
        args = b"\x00\x01\x02"
        result = await client.query("state.Query", args)

        # Verify the result
        self.assertEqual(result, bytes.fromhex("deadbeef"))

        # Verify the API call uses the custom URL
        mock_client.post.assert_called_once_with(
            "https://rofl.example.com/rofl/v1/query",
            json={"method": "state.Query", "args": "000102"},
            timeout=60.0,
        )


if __name__ == "__main__":
    unittest.main()
