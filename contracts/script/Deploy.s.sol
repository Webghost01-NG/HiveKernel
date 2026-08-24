// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../HiveEscrow.sol";

contract DeployHiveEscrow {
    function run() external returns (HiveEscrow) {
        HiveEscrow escrow = new HiveEscrow();
        return escrow;
    }
}
