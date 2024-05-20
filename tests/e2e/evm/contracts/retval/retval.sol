pragma solidity ^0.8.0;

contract Test {
    function testSuccess() external view returns (bytes memory) {
        bytes memory data = new bytes(1050); // Over the 1024 byte limit.
        data[0] = 0xFF;
        data[959] = 0x42; // Offset of 64 bytes.
        data[1049] = 0xFF;
        return data;
    }

    function testRevert() external view {
        bytes memory data = new bytes(1050); // Over the 1024 byte limit.
        data[0] = 0xFF;
        data[955] = 0x42; // Offset of 68 bytes.
        data[1049] = 0xFF;
        require(false, string(data));
    }
}
