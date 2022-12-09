pragma solidity ^0.8.0;

contract Test {
    address constant private RANDOM_BYTES = 0x0100000000000000000000000000000000000001;

    function randomBytes(uint256 count, bytes memory pers) internal view returns (bool, bytes memory) {
        return RANDOM_BYTES.staticcall(abi.encode(count, pers));
    }

    function test() external view {
        // Generate a normal amount of bytes with no personalization.
        (bool success1, bytes memory out1) = randomBytes(10, bytes("personalized!"));
        require(success1, "unsuccessful1");
        require(out1.length == 10, "bad length1");
        bytes memory zeros = new bytes(10);
        require(keccak256(out1) != keccak256(zeros), "1=0");

        // Generate some more bytes and make sure they don't match.
        (bool success2, bytes memory out2) = randomBytes(10, "");
        (bool success3, bytes memory out3) = randomBytes(10, "");
        require(success2 && success3 && out2.length == out3.length, "2&3");
        require(keccak256(out1) != keccak256(out2), "1=2");
        require(keccak256(out2) != keccak256(out3), "2=3");

        // Generate too many bytes.
        (bool success4, bytes memory out4) = randomBytes(1234567, "");
        require(success4, "unsuccessful4");
        require(out4.length == 1024, "bad length 4");
    }
}
