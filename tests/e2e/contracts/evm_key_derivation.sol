pragma solidity ^0.8.0;

contract Test {
    constructor() {}
    function test(bytes32 key_public, bytes32 key_private, bytes32 expected_symmetric) public view returns (uint) {
        bytes32[3] memory data;
        data[0] = key_public;
        data[1] = key_private;
        assembly {
            let success := staticcall(gas(), 0x0100000000000000000000000000000000000002, data, 0x40, add(data, 0x40), 0x20)
            if iszero(success) {
                revert(0, 0)
            }
        }
        if (data[2] == expected_symmetric) {
            return 0;
        }
        return uint(data[2]);
    }
}
