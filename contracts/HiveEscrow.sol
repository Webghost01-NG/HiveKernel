// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title HiveEscrow
 * @dev Optimistic Escrow & Multi-Agent Dispute Settlement Contract for HiveKernel.
 *      Designed for Swarm Village (HERŌ Network / Web3Bridge Hackathon).
 */
contract HiveEscrow {
    enum TaskStatus { Pending, Assigned, AwaitingValidation, Disputed, Settled, Slashed }

    struct Task {
        bytes32 taskId;
        address delegator;
        address worker;
        uint256 bounty;
        uint256 challengeDeadline;
        bytes32 executionDigest;
        TaskStatus status;
    }

    mapping(bytes32 => Task) public tasks;
    mapping(address => uint256) public agentStakes;

    event TaskCreated(bytes32 indexed taskId, address indexed delegator, uint256 bounty);
    event WorkerAssigned(bytes32 indexed taskId, address indexed worker);
    event ReceiptSubmitted(bytes32 indexed taskId, bytes32 executionDigest, uint256 challengeDeadline);
    event DisputeRaised(bytes32 indexed taskId, address indexed validator, string reason);
    event TaskSettled(bytes32 indexed taskId, address indexed worker, uint256 payout);
    event WorkerSlashed(bytes32 indexed taskId, address indexed delegator, uint256 refund);

    error TaskAlreadyExists();
    error TaskNotFound();
    error Unauthorized();
    error InvalidState();
    error ChallengeWindowActive();

    function createEscrow(bytes32 taskId) external payable {
        if (tasks[taskId].delegator != address(0)) revert TaskAlreadyExists();
        if (msg.value == 0) revert InvalidState();

        tasks[taskId] = Task({
            taskId: taskId,
            delegator: msg.sender,
            worker: address(0),
            bounty: msg.value,
            challengeDeadline: 0,
            executionDigest: bytes32(0),
            status: TaskStatus.Pending
        });

        emit TaskCreated(taskId, msg.sender, msg.value);
    }

    function assignWorker(bytes32 taskId, address worker) external {
        Task storage task = tasks[taskId];
        if (task.delegator != msg.sender) revert Unauthorized();
        if (task.status != TaskStatus.Pending) revert InvalidState();

        task.worker = worker;
        task.status = TaskStatus.Assigned;

        emit WorkerAssigned(taskId, worker);
    }

    function submitReceipt(bytes32 taskId, bytes32 executionDigest, uint256 challengePeriodSeconds) external {
        Task storage task = tasks[taskId];
        if (task.worker != msg.sender) revert Unauthorized();
        if (task.status != TaskStatus.Assigned) revert InvalidState();

        task.executionDigest = executionDigest;
        task.challengeDeadline = block.timestamp + challengePeriodSeconds;
        task.status = TaskStatus.AwaitingValidation;

        emit ReceiptSubmitted(taskId, executionDigest, task.challengeDeadline);
    }

    function raiseDispute(bytes32 taskId, string calldata reason) external {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.AwaitingValidation) revert InvalidState();
        if (block.timestamp > task.challengeDeadline) revert InvalidState();

        task.status = TaskStatus.Disputed;
        emit DisputeRaised(taskId, msg.sender, reason);
    }

    function finalizeSettlement(bytes32 taskId) external {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.AwaitingValidation) revert InvalidState();
        if (block.timestamp <= task.challengeDeadline) revert ChallengeWindowActive();

        task.status = TaskStatus.Settled;
        uint256 payout = task.bounty;
        task.bounty = 0;

        (bool success, ) = payable(task.worker).call{value: payout}("");
        require(success, "Payout transfer failed");

        emit TaskSettled(taskId, task.worker, payout);
    }

    function resolveDispute(bytes32 taskId, bool slashWorker) external {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.Disputed) revert InvalidState();

        if (slashWorker) {
            task.status = TaskStatus.Slashed;
            uint256 refund = task.bounty;
            task.bounty = 0;
            
            (bool success, ) = payable(task.delegator).call{value: refund}("");
            require(success, "Refund transfer failed");

            emit WorkerSlashed(taskId, task.delegator, refund);
        } else {
            task.status = TaskStatus.Settled;
            uint256 payout = task.bounty;
            task.bounty = 0;

            (bool success, ) = payable(task.worker).call{value: payout}("");
            require(success, "Payout transfer failed");

            emit TaskSettled(taskId, task.worker, payout);
        }
    }
}
