// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title MOMP email-addressed payments
/// @notice A multichain port of the original Sophia contract. Email values must be hashes.
contract Momp {
    error Unauthorized();
    error IncorrectFee();
    error InvalidState();
    error TransferFailed();
    error Reentrancy();

    struct EmailRecord {
        address account;
        bytes32 otpCommitment;
        bool verified;
        address pendingAccount;
        bytes32 pendingOtpCommitment;
    }

    struct Payment {
        address sender;
        bytes32 email;
        uint256 amount;
        uint64 expiresAt;
        bool claimed;
        bool refunded;
    }

    address public immutable owner;
    uint256 public registrationFee = 0.01 ether;
    uint256 public baseFee = 0.05 ether;
    uint64 public refundPeriod = 15 days;
    uint256 public feesCollected;
    uint256 public nonce;
    bool private locked;

    mapping(bytes32 => bool) public registrationFeePaid;
    mapping(bytes32 => EmailRecord) public emails;
    mapping(bytes32 => uint256) public escrowed;
    mapping(bytes32 => Payment) public payments;
    mapping(address => bytes32[]) private senderPayments;
    mapping(bytes32 => bytes32[]) private receiverPayments;

    event RegistrationFeePaid(bytes32 indexed email, address indexed payer);
    event EmailQueued(bytes32 indexed email, address indexed account, bool edit);
    event EmailVerified(bytes32 indexed email, address indexed account, bool edit);
    event PaymentCreated(bytes32 indexed paymentId, address indexed sender, bytes32 indexed email, uint256 amount, bool delivered, bytes queryData);
    event RefundClaimed(bytes32 indexed paymentId, address indexed sender, uint256 amount);
    event EscrowWithdrawn(bytes32 indexed email, address indexed recipient, uint256 amount);

    modifier onlyOwner() { if (msg.sender != owner) revert Unauthorized(); _; }
    modifier nonReentrant() { if (locked) revert Reentrancy(); locked = true; _; locked = false; }

    constructor() { owner = msg.sender; }

    function payRegistrationFee(bytes32 email) external payable {
        if (msg.value != registrationFee) revert IncorrectFee();
        registrationFeePaid[email] = true;
        feesCollected += msg.value;
        emit RegistrationFeePaid(email, msg.sender);
    }

    function addEmail(bytes32 email, address account, bytes32 otpCommitment) external onlyOwner {
        if (!registrationFeePaid[email] || account == address(0) || otpCommitment == bytes32(0)) revert InvalidState();
        EmailRecord storage record = emails[email];
        bool edit = record.account != address(0);
        if (edit) {
            record.pendingAccount = account;
            record.pendingOtpCommitment = otpCommitment;
        } else {
            record.account = account;
            record.otpCommitment = otpCommitment;
        }
        emit EmailQueued(email, account, edit);
    }

    function verifyOtp(bytes32 email, string calldata otp) external {
        EmailRecord storage record = emails[email];
        bool edit = record.pendingAccount != address(0);
        address candidate = edit ? record.pendingAccount : record.account;
        bytes32 expected = edit ? record.pendingOtpCommitment : record.otpCommitment;
        if (candidate == address(0) || sha256(abi.encodePacked(email, otp, candidate)) != expected) revert InvalidState();
        if (edit && msg.sender != record.account) revert Unauthorized();
        record.account = candidate;
        record.otpCommitment = expected;
        record.verified = true;
        record.pendingAccount = address(0);
        record.pendingOtpCommitment = bytes32(0);
        registrationFeePaid[email] = false;
        emit EmailVerified(email, candidate, edit);
    }

    function sendMoney(bytes32 email, bytes calldata queryData) external payable nonReentrant returns (bytes32 paymentId) {
        if (msg.value <= baseFee) revert IncorrectFee();
        uint256 amount = msg.value - baseFee;
        feesCollected += baseFee;
        paymentId = keccak256(abi.encodePacked(block.chainid, address(this), msg.sender, email, nonce++));
        EmailRecord storage record = emails[email];
        bool delivered = record.verified;
        payments[paymentId] = Payment(msg.sender, email, amount, uint64(block.timestamp) + refundPeriod, delivered, false);
        senderPayments[msg.sender].push(paymentId);
        receiverPayments[email].push(paymentId);
        if (delivered) _send(record.account, amount);
        else escrowed[email] += amount;
        emit PaymentCreated(paymentId, msg.sender, email, amount, delivered, queryData);
    }

    function claimRefund(bytes32 paymentId) external nonReentrant {
        Payment storage payment = payments[paymentId];
        if (payment.sender != msg.sender || payment.claimed || payment.refunded || block.timestamp < payment.expiresAt) revert InvalidState();
        uint256 amount = payment.amount;
        payment.refunded = true;
        payment.amount = 0;
        escrowed[payment.email] -= amount;
        _send(msg.sender, amount);
        emit RefundClaimed(paymentId, msg.sender, amount);
    }

    function withdrawEscrow(bytes32 email) external nonReentrant {
        EmailRecord storage record = emails[email];
        if (!record.verified || msg.sender != record.account) revert Unauthorized();
        uint256 amount = escrowed[email];
        if (amount == 0) revert InvalidState();
        escrowed[email] = 0;
        bytes32[] storage ids = receiverPayments[email];
        for (uint256 i; i < ids.length; ++i) {
            Payment storage payment = payments[ids[i]];
            if (!payment.claimed && !payment.refunded) payment.claimed = true;
        }
        _send(record.account, amount);
        emit EscrowWithdrawn(email, record.account, amount);
    }

    function withdrawFees(address payable recipient, uint256 amount) external onlyOwner nonReentrant {
        if (amount > feesCollected) revert InvalidState();
        feesCollected -= amount;
        _send(recipient, amount);
    }

    function setBaseFee(uint256 value) external onlyOwner { baseFee = value; }
    function setRegistrationFee(uint256 value) external onlyOwner { registrationFee = value; }
    function setRefundPeriod(uint64 value) external onlyOwner { refundPeriod = value; }
    function isVerifiedEmail(bytes32 email) external view returns (bool) { return emails[email].verified; }
    function paymentIdsForSender(address sender) external view returns (bytes32[] memory) { return senderPayments[sender]; }
    function paymentIdsForReceiver(bytes32 email) external view returns (bytes32[] memory) { return receiverPayments[email]; }

    function _send(address recipient, uint256 amount) private {
        (bool ok,) = payable(recipient).call{value: amount}("");
        if (!ok) revert TransferFailed();
    }
}
