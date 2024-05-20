pragma solidity ^0.8.0;

contract Test {
    address private constant GAS_USED =
        0x0100000000000000000000000000000000000009;
    address private constant PADGAS =
        0x010000000000000000000000000000000000000a;

    event UsedGas(uint256 value);

    function test_gas_used() public {
        (bool success, bytes memory gas_used) = GAS_USED.call("");
        require(success, "gas_used call failed");
        emit UsedGas(abi.decode(gas_used, (uint256)));
    }

    modifier padGas(uint128 amount) {
        _;
        (bool success, bytes memory data) = PADGAS.call(abi.encode(amount));
        require(success, "padgas failed");
    }

    function test_pad_gas(
        uint128 input
    ) public padGas(20_000) returns (uint128) {
        if (input > 10) {
            input = (input / 2) - 5;
            return input;
        } else {
            return input;
        }
    }
}
