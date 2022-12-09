pragma solidity ^0.8.0;

contract Test {
    address private constant DERIVE_KEY = 0x0100000000000000000000000000000000000002;
    function deriveKey(bytes32 peerPublicKey, bytes32 secretKey) internal view returns (bytes32) {
        (bool success, bytes memory symmetric) = DERIVE_KEY.staticcall(abi.encode(peerPublicKey, secretKey));
        require(success, "unsuccessful");
        require(symmetric.length == 32, "bad length");
        return bytes32(symmetric);
    }

    function test(bytes32 key_public, bytes32 key_private, bytes32 expected_symmetric) public view returns (uint) {
        bytes32 symmetric = deriveKey(key_public, key_private);
        require(symmetric == expected_symmetric, "mismatch");
        return uint(symmetric);
    }
}
