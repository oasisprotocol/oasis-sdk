from enum import Enum

from web3.types import TxParams


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


ROFL_SOCKET_PATH = "/run/rofl-appd.sock"

ENDPOINT_APP_ID = "/rofl/v1/app/id"
ENDPOINT_KEYS_GENERATE = "/rofl/v1/keys/generate"
ENDPOINT_TX_SIGN_SUBMIT = "/rofl/v1/tx/sign-submit"
ENDPOINT_METADATA = "/rofl/v1/metadata"
ENDPOINT_QUERY = "/rofl/v1/query"


def get_tx_payload(tx: TxParams, encrypt: bool):
    """Prepare the payload of the EVM transaction for the "tx/sign-submit" appd endpoint.

    :param tx: Transaction parameters
    :param encrypt: End-to-end encrypt the transaction before submitting (default: True)
    :returns: The payload object
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

    return payload
