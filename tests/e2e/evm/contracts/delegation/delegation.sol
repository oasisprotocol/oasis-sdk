// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Test {
    string private constant CONSENSUS_DELEGATE = "consensus.Delegate";
    string private constant CONSENSUS_UNDELEGATE = "consensus.Undelegate";
    string private constant CONSENSUS_TAKE_RECEIPT = "consensus.TakeReceipt";
    string private constant CONSENSUS_DELEGATION = "consensus.Delegation";
    string private constant CONSENSUS_SHARES_TO_TOKENS = "consensus.SharesToTokens";

    uint8 private constant RECEIPT_KIND_DELEGATE = 1;
    uint8 private constant RECEIPT_KIND_UNDELEGATE_START = 2;
    uint8 private constant RECEIPT_KIND_UNDELEGATE_DONE = 3;

    address private constant SUBCALL = 0x0100000000000000000000000000000000000103;

    // NOTE: All receipt identifiers are uint8 and <= 23 to simplify CBOR encoding/decoding.
    uint8 lastReceiptId;

    // receiptId => PendingDelegation
    mapping(uint8 => PendingDelegation) pendingDelegations;
    // (from, to) => shares
    mapping(address => mapping(bytes => uint128)) delegations;

    // receiptId => PendingUndelegation
    mapping(uint8 => PendingUndelegation) pendingUndelegations;
    // endReceiptId => UndelegationPool
    mapping(uint8 => UndelegationPool) undelegationPools;

    struct PendingDelegation {
        address from;
        bytes to;
        uint128 amount;
    }

    struct PendingUndelegation {
        bytes from;
        address payable to;
        uint128 shares;
        uint8 endReceiptId;
    }

    struct UndelegationPool {
        uint128 totalShares;
        uint128 totalAmount;
    }

    error SubcallFailed(uint64 code, bytes module);

    function delegate(bytes calldata to) public payable returns (uint64) {
        // Whatever is sent to the contract is delegated.
        uint128 amount = uint128(msg.value);

        lastReceiptId++;
        uint8 receiptId = lastReceiptId;
        require(receiptId <= 23, "receipt identifier overflow"); // Because our CBOR encoder is lazy.

        // Delegate to target address.
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(
            CONSENSUS_DELEGATE,
            abi.encodePacked(
                hex"a362",
                "to",
                hex"55",
                to,
                hex"66",
                "amount",
                hex"8250",
                amount,
                hex"4067",
                "receipt",
                receiptId // Only works for values <= 23.
            )
        ));
        require(success, "delegate subcall failed");
        (uint64 status, bytes memory result) = abi.decode(data, (uint64, bytes));
        if (status != 0) {
            revert SubcallFailed(status, result);
        }

        pendingDelegations[receiptId] = PendingDelegation(msg.sender, to, amount);

        return receiptId;
    }

    function delegation(bytes calldata from, bytes calldata to) public returns (bytes memory) {
        require(from.length == 21, "from address must be 21 bytes long");
        require(to.length == 21, "to address must be 21 bytes long");

        (bool success, bytes memory data) = SUBCALL.call(abi.encode(
            CONSENSUS_DELEGATION,
            // Manually encode CBOR for the DelegationQuery argument struct.
            abi.encodePacked(
                hex"a262", // map(2) + text(2)
                "to",      // The "to" field comes first, since our CBOR is deterministic.
                hex"55",   // Should be 95 (array(21)), but only map seems to work...
                to,
                hex"64",   // text(4)
                "from",
                hex"55",   // Should be 95 (array(21)), but only map seems to work...
                from
            )
        ));
        require(success, "delegation subcall failed");
        (uint64 status, bytes memory result) = abi.decode(data, (uint64, bytes));
        if (status != 0) {
            revert SubcallFailed(status, result);
        }

        return result;
    }

    function sharesToTokens(bytes calldata addr, uint8 pool, uint128 shares) public returns (uint128) {
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(
            CONSENSUS_SHARES_TO_TOKENS,
            // Manually encode CBOR for the SharesToTokens argument struct.
            abi.encodePacked(
                hex"a364", // map(3) + text(4)
                "pool",
                pool,      // Only works for values <= 23.
                hex"66",   // text(6)
                "shares",
                hex"50",
                shares,
                hex"67",   // text(7)
                "address",
                hex"55",
                addr
            )
        ));
        require(success, "sharesToTokens subcall failed");
        (uint64 status, bytes memory result) = abi.decode(data, (uint64, bytes));
        if (status != 0) {
            revert SubcallFailed(status, result);
        }

        // We'd have to decode CBOR here, but this hack works well enough for the test...
        // Double cast required because Solidity is dumb.
        return uint128(uint8(result[1]));
    }

    function takeReceipt(uint8 kind, uint8 receiptId) internal returns (bytes memory) {
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(
            CONSENSUS_TAKE_RECEIPT,
            abi.encodePacked(
                hex"a262",
                "id",
                receiptId, // Only works for values <= 23.
                hex"64",
                "kind",
                kind // Only works for values <= 23.
            )
        ));
        require(success, "take receipt subcall failed");
        (uint64 status, bytes memory result) = abi.decode(data, (uint64, bytes));
        if (status != 0) {
            revert SubcallFailed(status, result);
        }

        return result;
    }

    function delegateDone(uint8 receiptId) public returns (uint128) {
        require(receiptId <= 23, "receipt identifier overflow"); // Because our CBOR encoder is lazy.

        PendingDelegation memory pending = pendingDelegations[receiptId];
        require(pending.from != address(0), "unknown receipt");

        bytes memory result = takeReceipt(RECEIPT_KIND_DELEGATE, receiptId);

        // This is a very lazy CBOR decoder. It assumes that if there is only a shares field then
        // the delegation operation succeeded and if not, there was some sort of error which is
        // not decoded.
        uint128 shares = 0;
        if (result[0] == 0xA1 && result[1] == 0x66 && result[2] == "s") {
            // Delegation succeeded, decode number of shares.
            uint8 sharesLen = uint8(result[8]) & 0x1f; // Assume shares field is never greater than 16 bytes.
            for (uint offset = 0; offset < sharesLen; offset++) {
                uint8 v = uint8(result[9 + offset]);
                shares += uint128(v) << 8*uint128(sharesLen - offset - 1);
            }

            // Add to given number of shares.
            delegations[pending.from][pending.to] += shares;
        } else {
            // Should refund the owner in case of errors. This just keeps the funds.
        }

        // Remove pending delegation.
        delete(pendingDelegations[receiptId]);

        return shares;
    }

    function undelegate(bytes calldata from, uint128 shares) public returns (uint64) {
        require(shares > 0, "must undelegate some shares");
        require(delegations[msg.sender][from] >= shares, "must have enough delegated shares");

        lastReceiptId++;
        uint8 receiptId = lastReceiptId;
        require(receiptId <= 23, "receipt identifier overflow"); // Because our CBOR encoder is lazy.

        // Start undelegation from source address.
        (bool success, bytes memory data) = SUBCALL.call(abi.encode(
            CONSENSUS_UNDELEGATE,
            abi.encodePacked(
                hex"a364",
                "from",
                hex"55",
                from,
                hex"66",
                "shares",
                hex"50",
                shares,
                hex"67",
                "receipt",
                receiptId // Only works for values <= 23.
            )
        ));
        require(success, "undelegate subcall failed");
        (uint64 status, bytes memory result) = abi.decode(data, (uint64, bytes));
        if (status != 0) {
            revert SubcallFailed(status, result);
        }

        delegations[msg.sender][from] -= shares;
        pendingUndelegations[receiptId] = PendingUndelegation(from, payable(msg.sender), shares, 0);

        return receiptId;
    }

    function undelegateStart(uint8 receiptId) public {
        require(receiptId <= 23, "receipt identifier overflow"); // Because our CBOR encoder is lazy.

        PendingUndelegation memory pending = pendingUndelegations[receiptId];
        require(pending.to != address(0), "unknown receipt");

        bytes memory result = takeReceipt(RECEIPT_KIND_UNDELEGATE_START, receiptId);

        // This is a very lazy CBOR decoder. It assumes that if there are only an epoch and receipt fields
        // then the undelegation operation succeeded and if not, there was some sort of error which is not
        // decoded.
        if (result[0] == 0xA2 && result[1] == 0x65 && result[2] == "e" && result[3] == "p") {
            // Undelegation started, decode end epoch (only supported up to epoch 255).
            uint64 epoch = 0;
            uint8 fieldOffset = 7;
            uint8 epochLow = uint8(result[fieldOffset]) & 0x1f;
            if (epochLow <= 23) {
                epoch = uint64(epochLow);
                fieldOffset++;
            } else if (epochLow == 24) {
                epoch = uint64(uint8(result[fieldOffset + 1]));
                fieldOffset += 2;
                require(epoch >= 24, "malformed epoch in receipt");
            } else {
                // A real implementation would support decoding bigger epoch numbers.
                revert("unsupported epoch length");
            }

            // Decode end receipt identifier.
            require(result[fieldOffset] == 0x67 && result[fieldOffset + 1] == "r", "malformed receipt");
            uint8 endReceipt = uint8(result[fieldOffset + 8]) & 0x1f; // Assume end receipt is never greater than 23.

            pendingUndelegations[receiptId].endReceiptId = endReceipt;
            undelegationPools[endReceipt].totalShares += pending.shares;
        } else {
            // Undelegation failed to start, return the shares.
            delegations[msg.sender][pending.from] += pending.shares;
            delete(pendingUndelegations[receiptId]);
        }
    }

    function undelegateDone(uint8 receiptId) public {
        require(receiptId <= 23, "receipt identifier overflow"); // Because our CBOR encoder is lazy.

        PendingUndelegation memory pending = pendingUndelegations[receiptId];
        require(pending.to != address(0), "unknown receipt");
        require(pending.endReceiptId > 0, "must call undelegateStart first");

        UndelegationPool memory pool = undelegationPools[pending.endReceiptId];
        if (pool.totalAmount == 0) {
            // Did not fetch the end receipt yet, do it now.
            bytes memory result = takeReceipt(RECEIPT_KIND_UNDELEGATE_DONE, pending.endReceiptId);

            // This is a very lazy CBOR decoder. It assumes that if there is only an amount field then
            // the undelegation operation succeeded and if not, there was some sort of error which is
            // not decoded.
            uint128 amount = 0;
            if (result[0] == 0xA1 && result[1] == 0x66 && result[2] == "a") {
                // Undelegation succeeded, decode token amount.
                uint8 amountLen = uint8(result[8]) & 0x1f; // Assume amount field is never greater than 16 bytes.
                for (uint offset = 0; offset < amountLen; offset++) {
                    uint8 v = uint8(result[9 + offset]);
                    amount += uint128(v) << 8*uint128(amountLen - offset - 1);
                }

                undelegationPools[pending.endReceiptId].totalAmount = amount;
                pool.totalAmount = amount;
            } else {
                // Should never fail.
                revert("undelegation failed");
            }
        }

        // Compute how much we get from the pool and transfer the amount.
        uint128 transferAmount = (pending.shares * pool.totalAmount) / pool.totalShares;
        pending.to.transfer(transferAmount);

        delete(pendingUndelegations[receiptId]);
    }
}
