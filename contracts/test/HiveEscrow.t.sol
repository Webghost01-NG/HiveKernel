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
    address arbiter;
    address delegator = address(0x1111);
    address worker = address(0x2222);
    address validator = address(0x3333);
    address unauthorizedAttacker = address(0x9999);

    bytes32 taskId = keccak256("TASK_SECURITY_AUDIT_001");
    bytes32 executionDigest = keccak256("DIGEST_VALID_AUDIT_OUTPUT");
    uint256 requiredStake = 0.2 ether;

    function setUp() public {
        arbiter = address(this);
        escrow = new HiveEscrow();

        vm.deal(delegator, 10 ether);
        vm.deal(worker, 5 ether);
        vm.deal(validator, 5 ether);
        vm.deal(unauthorizedAttacker, 5 ether);

        // Worker deposits collateral stake
        vm.prank(worker);
        escrow.depositStake{value: 1 ether}();

        // Validator deposits collateral stake
        vm.prank(validator);
        escrow.depositStake{value: 0.5 ether}();
    }

    function testCreateAndAssignTaskWithStaking() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId, requiredStake);

        (
            bytes32 id,
            address del,
            address wrk,
            ,
            uint256 bty,
            uint256 reqStake,
            ,
            ,
            HiveEscrow.TaskStatus status
        ) = escrow.tasks(taskId);

        require(id == taskId, "TaskId mismatch");
        require(del == delegator, "Delegator mismatch");
        require(wrk == address(0), "Worker should be empty");
        require(bty == 1 ether, "Bounty mismatch");
        require(reqStake == requiredStake, "Required stake mismatch");
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Pending), "Should be pending");

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        (, , wrk, , , , , , status) = escrow.tasks(taskId);
        require(wrk == worker, "Worker assignment failed");
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Assigned), "Should be assigned");
        require(escrow.lockedStakes(worker) == requiredStake, "Worker stake was not locked");
    }

    function testOptimisticSettlementUnlocksStakeAndPaysWorker() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId, requiredStake);

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        vm.prank(worker);
        escrow.submitReceipt(taskId, executionDigest, 60);

        // Advance block timestamp beyond challenge deadline
        vm.warp(block.timestamp + 61);

        uint256 initialBal = worker.balance;
        escrow.finalizeSettlement(taskId);

        require(worker.balance == initialBal + 1 ether, "Worker was not paid bounty");
        require(escrow.lockedStakes(worker) == 0, "Worker stake was not unlocked");

        (, , , , , , , , HiveEscrow.TaskStatus status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Settled), "Status should be settled");
    }

    function testDisputeAndSlashingWorkerStake() public {
        vm.prank(delegator);
        escrow.createEscrow{value: 1 ether}(taskId, requiredStake);

        vm.prank(delegator);
        escrow.assignWorker(taskId, worker);

        vm.prank(worker);
        escrow.submitReceipt(taskId, executionDigest, 60);

        // Staked validator raises dispute
        vm.prank(validator);
        escrow.raiseDispute(taskId, "Fraudulent audit findings digest");

        (, , , , , , , , HiveEscrow.TaskStatus status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Disputed), "Status should be disputed");

        // Unauthorized attacker cannot resolve dispute
        vm.prank(unauthorizedAttacker);
        try escrow.resolveDispute(taskId, true) {
            revert("Unauthorized address was able to resolve dispute!");
        } catch {}

        uint256 delegatorInitial = delegator.balance;
        uint256 validatorInitial = validator.balance;
        uint256 workerInitialStake = escrow.agentStakes(worker);

        // Arbiter resolves dispute: slashes worker
        escrow.resolveDispute(taskId, true);

        require(escrow.agentStakes(worker) == workerInitialStake - requiredStake, "Worker stake was not slashed");
        require(delegator.balance > delegatorInitial, "Delegator did not receive refund and compensation");
        require(validator.balance > validatorInitial, "Validator did not receive reward");

        (, , , , , , , , status) = escrow.tasks(taskId);
        require(uint8(status) == uint8(HiveEscrow.TaskStatus.Slashed), "Status should be slashed");
    }
}
