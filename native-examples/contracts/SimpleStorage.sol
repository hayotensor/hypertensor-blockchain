// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice An example anyone can update. The public variable provides value().
contract SimpleStorage {
    uint256 public value;

    event ValueUpdated(uint256 previousValue, uint256 newValue);

    constructor(uint256 initialValue) {
        value = initialValue;
    }

    function setValue(uint256 newValue) external {
        uint256 previousValue = value;
        value = newValue;
        emit ValueUpdated(previousValue, newValue);
    }
}
