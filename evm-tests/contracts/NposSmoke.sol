// SPDX-License-Identifier: MIT
pragma solidity ^0.8.25;

/// A minimal fixture for signed deployment, storage, eth_call and indexed logs.
contract NposSmoke {
    uint256 public value;
    event Changed(address indexed sender, uint256 value);

    function set(uint256 nextValue) external {
        value = nextValue;
        emit Changed(msg.sender, nextValue);
    }
}
