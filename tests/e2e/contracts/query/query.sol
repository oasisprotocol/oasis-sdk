pragma solidity ^0.8.0;

contract Test {
    function test() public view returns (address) {
        return msg.sender;
    }
}
