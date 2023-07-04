pragma solidity ^0.8.0;

contract Test {
    address private constant SUBCALL = 0x0100000000000000000000000000000000000102;

    error SubcallFailed(uint64 code, bytes module);

    function test(bytes calldata method, bytes calldata body) public payable returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(method, body));
        require(success, "subcall failed");
        return decodeResponse(data);
    }

    function test_delegatecall(bytes calldata method, bytes calldata body) public returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.delegatecall(abi.encode(method, body));
        require(success, "subcall failed");
        return data;
    }

    function test_spin(bytes calldata method, bytes calldata body) public returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(method, body));
        require(success, "subcall failed");
        for (int i = 0; i < 100; i++) {
            // Spin.
        }
        return data;
    }

    function decodeResponse(bytes memory raw) internal pure returns (bytes memory) {
        (uint64 status_code, bytes memory data) = abi.decode(raw, (uint64, bytes));

        if (status_code != 0) {
            revert SubcallFailed(status_code, data);
        }
        return data;
    }
}
