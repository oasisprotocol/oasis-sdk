pragma solidity ^0.8.0;

contract Test {
    constructor() {}
    function test() public view returns (bytes memory result) {
        assembly {
            let key := 0x7468697320697320746865206b6579207573656420666f722074657374696e67
            let plaintext := 0x6120706c61696e7465787420746f2072756c65207468656d20616c6c2c207961
            let additional := 0x6164000000000000000000000000000000000000000000000000000000000000
            let ciphertext_ptr := 0x0
            let ciphertext_len := 0x0
            let ciphertext_size := 0x0

            // Try encrypting.
            let buf := mload(0x40)
            mstore(buf, key) // Key.
            mstore(add(buf, 0x20), shl(248, 0x01)) // Nonce.
            mstore(add(buf, 0x40), 0x20) // Text length.
            mstore(add(buf, 0x60), 0x02) // Additional data length.
            mstore(add(buf, 0x80), plaintext)
            mstore(add(buf, 0xa0), additional)
            let success := staticcall(gas(), 0x0100000000000000000000000000000000000003, buf, 192, buf, 192)
            switch success
            case 0 {
                revert(0, 0)
            } default {
                // Store encrypted bytes in memory.
                ciphertext_len := returndatasize()
                ciphertext_size := and(add(ciphertext_len, 0x1f), not(0x1f))
                mstore(sub(add(buf, ciphertext_size), 0x20), 0) // Zero out the last slot.
                returndatacopy(buf, 0, ciphertext_len)
                mstore(0x40, add(buf, add(ciphertext_size, 0x20)))
                ciphertext_ptr := buf
            }
            let input_len := add(add(ciphertext_size, 0x80), 0x20)

            // Try decrypting corrupt ciphertext.
            buf := mload(0x40)
            mstore(buf, key) // Key.
            mstore(add(buf, 0x20), shl(248, 0x01)) // Nonce.
            mstore(add(buf, 0x40), ciphertext_len) // Ciphertext length.
            mstore(add(buf, 0x60), 0x02) // Additional data length.
            let i := 0
            for {} lt(i, ciphertext_size) { i := add(i, 0x20) } {
                mstore(add(add(buf, 0x80), i), mload(add(ciphertext_ptr, i)))
            }
            let tmp := mload(add(buf, 0x80))
            mstore(add(buf, 0x80), add(tmp, 0x20))
            mstore(add(add(buf, 0x80), ciphertext_size), additional)
            success := staticcall(100000, 0x0100000000000000000000000000000000000004, buf, input_len, buf, 32)
            if success {
                revert(0, 0)
            }

            // Fix ciphertext but send corrupted additional data.
            mstore(buf, key) // Key.
            mstore(add(buf, 0x20), shl(248, 0x01)) // Nonce.
            mstore(add(buf, 0x40), ciphertext_len) // Ciphertext length.
            mstore(add(buf, 0x60), 0x02) // Additional data length.
            i := 0
            for {} lt(i, ciphertext_size) { i := add(i, 0x20) } {
                mstore(add(add(buf, 0x80), i), mload(add(ciphertext_ptr, i)))
            }
            mstore(add(add(buf, 0x80), ciphertext_size), not(additional))
            success := staticcall(100000, 0x0100000000000000000000000000000000000004, buf, input_len, buf, 0x20)
            if success {
                revert(0, 0)
            }

            // Correct ciphertext and correct additional data.
            mstore(add(add(buf, 0x80), ciphertext_size), additional)
            success := staticcall(gas(), 0x0100000000000000000000000000000000000004, buf, input_len, add(buf, 0x20), 0x20)
            switch success
            case 0 {
                revert(0, 0)
            } default {
                if iszero(eq(returndatasize(), 0x20)) {
                    revert(0, 0)
                }
                if iszero(eq(mload(add(buf, 0x20)), plaintext)) {
                    revert(0, 0)
                }
                // If ok, the return is 32 bytes, so no extra copying necessary.
                mstore(buf, returndatasize())
                mstore(0x40, add(buf, 0x40))
                result := buf
            }
        }
    }
}
