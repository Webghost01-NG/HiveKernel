// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../HiveEscrow.sol";

interface Vm {
    function warp(uint256) external;
    function deal(address who, uint256 newBalance) external;
    function prank(address) external;
}

contract HiveEscrowTest {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    
    HiveEscrow escrow;
    address delegator = address(0x1111);
    address worker = address(0x2222);
    address validator = address(0x3333);

    bytes32 taskId = keccak256("TASK_SECURITY_AUDIT_001");
    bytes32 executionDigest = keccak256("DIGEST_VALID_AUDIT_OUTPUT");

    function setUp() public {
        escrow = new HiveEscrow();
        vm.deal(delegator, 10 ether);
        vm.deal(worker, 1 ether);
        vm.deal(validator, 1 ether);
    }

    function testCreateAndAssignTask() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId);

        (bytes32 id, address del, address wrk, uint256 bty, , , HiveEscrow.TaskStatus status) = escrow.tasks(taskId);
        require(id == taskId, "TaskId mismatch");
        require(del == delegator, "Delegator mismatch");
        require(wrk == address(0), "Worker should be empty");
        require(bty == 1 ether, "Bounty mismatch");
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Pending), "Should be pending");

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        (, , wrk, , , , status) = escrow.tasks(taskId);
        require(wrk == worker, "Worker assignment failed");
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Assigned), "Should be assigned");
    }

    function testOptimisticSettlement() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId);

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        vm.prank(worker);
        escrow.submitReceipt(taskId, executionDigest, 60);

        // Advance block timestamp beyond challenge deadline
        vm.warp(block.timestamp + 61);

        uint256 initialBal = worker.balance;
        escrow.finalizeSettlement(taskId);

        require(worker.balance == initialBal + 1 ether, "Worker was not paid");
        (, , , , , , HiveEscrow.TaskStatus status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Settled), "Status should be settled");
    }

    function testDisputeAndSlashing() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId);

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        vm.prank(worker);
        escrow.submitReceipt(taskId, executionDigest, 60);

        // Validator raises dispute within challenge window
        vm.prank(validator);
        escrow.raiseDispute(taskId, "Fraudulent analysis digest detected");

        (, , , , , , HiveEscrow.TaskStatus status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Disputed), "Status should be disputed");

        uint256 delegatorInitial = delegator.balance;
        escrow.resolveDispute(taskId, true); // Slash worker, refund delegator

        require(delegator.balance == delegatorInitial + 1 ether, "Delegator was not refunded");
        (, , , , , , status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Slashed), "Status should be slashed");
    }
}
