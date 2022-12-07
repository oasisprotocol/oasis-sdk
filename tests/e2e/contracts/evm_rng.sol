pragma solidity ^0.8.0;

contract Test {
    constructor() {}
    function test() public view {
        assembly {
            // Generate some bytes.
            let num_words := 0x2a
            let num_bytes := mul(num_words, 0x20)
            let buf := mload(0x40)
            mstore(buf, num_words)
            let status := staticcall(gas(), 0x0100000000000000000000000000000000000001, buf, 0x20, buf, 0)
            if eq(status, 0) {
                revert(0, 0)
            }
            if not(eq(returndatasize(), num_bytes)) {
                revert(0, 0)
            }
            returndatacopy(buf, 0, num_bytes)
            // Make sure that the output isn't obviously buggy.
            let output_sum := 0
            for { let i := 0 } lt(i, num_words) { i := add(i, 0x20) } {
                output_sum := add(output_sum, mload(add(buf, i)))
            }
            if eq(output_sum, 0) {
                revert(0, 0)
            }

            // Generate an unreasonable number of bytes.
            num_words := 0xffff
            num_bytes := mul(num_words, 0x20)
            mstore(buf, num_words)
            status := staticcall(gas(), 0x0100000000000000000000000000000000000001, buf, 0x20, buf, num_bytes)
            if eq(status, 1) {
                revert(0, 0) // It shouldn't work because the number of bytes is unreasonable.
            }
        }
    }
}
