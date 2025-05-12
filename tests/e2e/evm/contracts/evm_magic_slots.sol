pragma solidity ^0.8.0;

contract Test {
    function setSlot(bytes32 slot, bytes32 value) external {
        assembly {
            sstore(slot, value)
        }
    }
}
