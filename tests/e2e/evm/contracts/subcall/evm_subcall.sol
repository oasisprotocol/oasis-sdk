pragma solidity ^0.8.0;

contract Test {
    bytes32 private constant TEST_RUNTIME_ID =
        0x8000000000000000000000000000000000000000000000000000000000000000;
    string private constant CONSENSUS_ROUND_ROOT = "consensus.RoundRoot";

    uint8 private constant ROOT_ROUND_KIND_STATE = 1;

    address private constant SUBCALL =
        0x0100000000000000000000000000000000000103;

    error SubcallFailed(uint64 code, bytes module);

    function test(
        bytes calldata method,
        bytes calldata body
    ) public payable returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(
            abi.encode(method, body)
        );
        require(success, "subcall failed");
        return decodeResponse(data);
    }

    function test_delegatecall(
        bytes calldata method,
        bytes calldata body
    ) public returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.delegatecall(
            abi.encode(method, body)
        );
        require(success, "subcall failed");
        return data;
    }

    function test_spin(
        bytes calldata method,
        bytes calldata body
    ) public returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(
            abi.encode(method, body)
        );
        require(success, "subcall failed");
        for (int i = 0; i < 100; i++) {
            // Spin.
        }
        return data;
    }

    function test_consensus_round_root() public returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(
            abi.encode(
                CONSENSUS_ROUND_ROOT,
                abi.encodePacked(
                    hex"a3",
                    hex"64",
                    "kind",
                    ROOT_ROUND_KIND_STATE, // Only works for values <= 23.
                    hex"65",
                    "round",
                    uint8(2), // Only works for values <= 23.
                    hex"6a",
                    "runtime_id",
                    hex"58",
                    hex"20",
                    TEST_RUNTIME_ID
                )
            )
        );
        require(success, "consensus round root subcall failed");
        (uint64 status, bytes memory result) = abi.decode(
            data,
            (uint64, bytes)
        );
        if (status != 0) {
            revert SubcallFailed(status, result);
        }
        return result;
    }

    function decodeResponse(
        bytes memory raw
    ) internal pure returns (bytes memory) {
        (uint64 status_code, bytes memory data) = abi.decode(
            raw,
            (uint64, bytes)
        );

        if (status_code != 0) {
            revert SubcallFailed(status_code, data);
        }
        return data;
    }
}
