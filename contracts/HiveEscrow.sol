// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title HiveEscrow
 * @dev Optimistic Subcontracting Escrow & Dispute Resolution Contract with Real Staking & Slashing.
 *      Built for Swarm Village (HERŌ Network / Web3Bridge Hackathon).
 */
contract HiveEscrow {
    enum TaskStatus {
        Pending,
        Assigned,
        AwaitingValidation,
        Disputed,
        Settled,
        Slashed
    }

    struct Task {
        bytes32 taskId;
        address delegator;
        address worker;
        address validator;
        uint256 bounty;
        uint256 requiredStake;
        uint256 challengeDeadline;
        bytes32 executionDigest;
        TaskStatus status;
    }

    address public immutable arbiter;
    uint256 public constant MIN_VALIDATOR_STAKE = 0.05 ether;

    mapping(bytes32 => Task) public tasks;
    mapping(address => uint256) public agentStakes;
    mapping(address => uint256) public lockedStakes;

    event StakeDeposited(address indexed agent, uint256 amount);
    event StakeWithdrawn(address indexed agent, uint256 amount);
    event TaskCreated(bytes32 indexed taskId, address indexed delegator, uint256 bounty, uint256 requiredStake);
    event WorkerAssigned(bytes32 indexed taskId, address indexed worker, uint256 lockedStake);
    event ReceiptSubmitted(bytes32 indexed taskId, bytes32 executionDigest, uint256 challengeDeadline);
    event DisputeRaised(bytes32 indexed taskId, address indexed validator, string reason);
    event TaskSettled(bytes32 indexed taskId, address indexed worker, uint256 payout, uint256 unlockedStake);
    event WorkerSlashed(
        bytes32 indexed taskId,
        address indexed worker,
        address indexed delegator,
        uint256 refund,
        uint256 slashedStake
    );

    error TaskAlreadyExists();
    error TaskNotFound();
    error Unauthorized();
    error InvalidState();
    error ChallengeWindowActive();
    error InsufficientStake();
    error TransferFailed();

    modifier onlyArbiter() {
        if (msg.sender != arbiter) revert Unauthorized();
        _;
    }

    constructor() {
        arbiter = msg.sender;
    }

    /// @notice Allows agents (workers and validators) to deposit collateral stake
    function depositStake() external payable {
        if (msg.value == 0) revert InvalidState();
        agentStakes[msg.sender] += msg.value;
        emit StakeDeposited(msg.sender, msg.value);
    }

    /// @notice Allows agents to withdraw unlocked collateral stake
    function withdrawStake(uint256 amount) external {
        uint256 available = agentStakes[msg.sender] - lockedStakes[msg.sender];
        if (amount > available) revert InsufficientStake();

        agentStakes[msg.sender] -= amount;
        (bool success, ) = payable(msg.sender).call{value: amount}("");
        if (!success) revert TransferFailed();

        emit StakeWithdrawn(msg.sender, amount);
    }

    /// @notice Delegator creates a task and locks bounty in escrow
    function createEscrow(bytes32 taskId, uint256 requiredStake) external payable {
        if (tasks[taskId].delegator != address(0)) revert TaskAlreadyExists();
        if (msg.value == 0) revert InvalidState();

        tasks[taskId] = Task({
            taskId: taskId,
            delegator: msg.sender,
            worker: address(0),
            validator: address(0),
            bounty: msg.value,
            requiredStake: requiredStake,
            challengeDeadline: 0,
            executionDigest: bytes32(0),
            status: TaskStatus.Pending
        });

        emit TaskCreated(taskId, msg.sender, msg.value, requiredStake);
    }

    /// @notice Delegator assigns an approved worker with sufficient collateral stake
    function assignWorker(bytes32 taskId, address worker) external {
        Task storage task = tasks[taskId];
        if (task.delegator != msg.sender) revert Unauthorized();
        if (task.status != TaskStatus.Pending) revert InvalidState();

        uint256 availableStake = agentStakes[worker] - lockedStakes[worker];
        if (availableStake < task.requiredStake) revert InsufficientStake();

        lockedStakes[worker] += task.requiredStake;
        task.worker = worker;
        task.status = TaskStatus.Assigned;

        emit WorkerAssigned(taskId, worker, task.requiredStake);
    }

    /// @notice Worker submits signed execution digest and opens optimistic challenge window
    function submitReceipt(
        bytes32 taskId,
        bytes32 executionDigest,
        uint256 challengePeriodSeconds
    ) external {
        Task storage task = tasks[taskId];
        if (task.worker != msg.sender) revert Unauthorized();
        if (task.status != TaskStatus.Assigned) revert InvalidState();

        task.executionDigest = executionDigest;
        task.challengeDeadline = block.timestamp + challengePeriodSeconds;
        task.status = TaskStatus.AwaitingValidation;

        emit ReceiptSubmitted(taskId, executionDigest, task.challengeDeadline);
    }

    /// @notice Staked Validator raises a fraud dispute before challenge deadline expires
    function raiseDispute(bytes32 taskId, string calldata reason) external {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.AwaitingValidation) revert InvalidState();
        if (block.timestamp > task.challengeDeadline) revert InvalidState();

        // Validator must have staked minimum collateral to prevent spam
        uint256 validatorAvailable = agentStakes[msg.sender] - lockedStakes[msg.sender];
        if (validatorAvailable < MIN_VALIDATOR_STAKE) revert InsufficientStake();

        task.validator = msg.sender;
        task.status = TaskStatus.Disputed;

        emit DisputeRaised(taskId, msg.sender, reason);
    }

    /// @notice Optimistic settlement: Releases bounty and unlocks worker stake after window passes
    function finalizeSettlement(bytes32 taskId) external {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.AwaitingValidation) revert InvalidState();
        if (block.timestamp <= task.challengeDeadline) revert ChallengeWindowActive();

        task.status = TaskStatus.Settled;
        uint256 payout = task.bounty;
        uint256 stakeToUnlock = task.requiredStake;
        task.bounty = 0;

        lockedStakes[task.worker] -= stakeToUnlock;

        (bool success, ) = payable(task.worker).call{value: payout}("");
        if (!success) revert TransferFailed();

        emit TaskSettled(taskId, task.worker, payout, stakeToUnlock);
    }

    /// @notice Arbiter resolves dispute: slashes worker stake if fraudulent, or pays worker if valid
    function resolveDispute(bytes32 taskId, bool slashWorker) external onlyArbiter {
        Task storage task = tasks[taskId];
        if (task.status != TaskStatus.Disputed) revert InvalidState();

        if (slashWorker) {
            task.status = TaskStatus.Slashed;
            uint256 refund = task.bounty;
            uint256 slashedAmount = task.requiredStake;
            task.bounty = 0;

            // Slash worker stake
            lockedStakes[task.worker] -= slashedAmount;
            agentStakes[task.worker] -= slashedAmount;

            // Refund delegator bounty + award half of slashed stake to delegator, half to validator
            uint256 validatorReward = slashedAmount / 2;
            uint256 delegatorCompensation = refund + (slashedAmount - validatorReward);

            if (task.validator != address(0) && validatorReward > 0) {
                (bool valSuccess, ) = payable(task.validator).call{value: validatorReward}("");
                if (!valSuccess) revert TransferFailed();
            }

            (bool delSuccess, ) = payable(task.delegator).call{value: delegatorCompensation}("");
            if (!delSuccess) revert TransferFailed();

            emit WorkerSlashed(taskId, task.worker, task.delegator, delegatorCompensation, slashedAmount);
        } else {
            task.status = TaskStatus.Settled;
            uint256 payout = task.bounty;
            uint256 stakeToUnlock = task.requiredStake;
            task.bounty = 0;

            lockedStakes[task.worker] -= stakeToUnlock;

            (bool success, ) = payable(task.worker).call{value: payout}("");
            if (!success) revert TransferFailed();

            emit TaskSettled(taskId, task.worker, payout, stakeToUnlock);
        }
    }
}
