pragma solidity ^0.8.0;

// Minimal faucet contract.
contract Faucet {
    // Accept any incoming amount.
    receive() external payable {}

    // Give out native tokens to anyone who asks.
    function withdraw(uint256 withdraw_amount) public {
        // Limit withdrawal amount.
        require(withdraw_amount <= 100000000000000000);

        // Send the amount to the address that requested it.
        payable(msg.sender).transfer(withdraw_amount);
    }
}
