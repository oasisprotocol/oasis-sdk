pragma solidity ^0.8.0;

contract Test {
    address constant private RANDOM_BYTES = 0x0100000000000000000000000000000000000001;

    function randomBytes(uint256 count, bytes memory pers) internal view returns (bool, bytes memory) {
        return RANDOM_BYTES.staticcall(abi.encode(count, pers));
    }

    function test() external view {
        // Generate a normal amount of bytes with no personalization.
        (bool success1, bytes memory out1) = randomBytes(10, bytes("personalized!"));
        require(success1, "1");
        uint256 outSum = 0;
        for(uint256 i = 0; i < out1.length; ++i) {
            outSum += uint256(uint8(out1[i]));
        }
        require(outSum > 0, "0");

        // Generate some more bytes and make sure they don't match.
        (bool success2, bytes memory out2) = randomBytes(10, "");
        require(success2, "2");
        bool allEq = true;
        for(uint256 i = 0; i < out1.length; ++i) {
            allEq = allEq && out1[i] == out2[i];
        }
        require(!allEq, "=");

        // Generate too many bytes.
        (bool success3, bytes memory out3) = randomBytes(500, "");
        require(success3 && out3.length < 4096, "!");
    }
}
