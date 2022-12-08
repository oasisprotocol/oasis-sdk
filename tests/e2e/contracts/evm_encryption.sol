pragma solidity ^0.8.0;

contract Test {
    address constant private ENCRYPT = 0x0100000000000000000000000000000000000003;
    address constant private DECRYPT = 0x0100000000000000000000000000000000000004;

    function encrypt(bytes32 key, bytes32 nonce, bytes memory plaintext, bytes memory ad) internal view returns (bool, bytes memory) {
        return ENCRYPT.staticcall(abi.encode(key, nonce, plaintext, ad));
    }

    function decrypt(bytes32 key, bytes32 nonce, bytes memory ciphertext, bytes memory ad) internal view returns (bool, bytes memory) {
        return DECRYPT.staticcall(abi.encode(key, nonce, ciphertext, ad));
    }

    function test() public view {
        bytes32 key = 0x7468697320697320746865206b6579207573656420666f722074657374696e67;
        bytes32 nonce = "good nonce";
        bytes memory plaintext = "a plaintext to rule them all, ya";
        bytes memory ad = "aa";

        (bool success1, bytes memory ciphertext) = encrypt(key, nonce, plaintext, ad);
        require(success1, "bad enc");
        (bool success2, bytes memory plaintext2) = decrypt(key, nonce, ciphertext, ad);
        require(success2, "bad dec");
        require(keccak256(plaintext) == keccak256(plaintext2), "pt != pt2");

        (bool success3,) = decrypt("bad key", nonce, ciphertext, ad);
        (bool success4,) = decrypt(key, "bad nonce", ciphertext, ad);
        (bool success5,) = decrypt(key, nonce, "bad ct", ad);
        (bool success6,) = decrypt(key, nonce, ciphertext, "bad ad");
        require(!success3, "bad key");
        require(!success4, "bad nonce");
        require(!success5, "bad ct");
        require(!success6, "bad ad");
    }
}
