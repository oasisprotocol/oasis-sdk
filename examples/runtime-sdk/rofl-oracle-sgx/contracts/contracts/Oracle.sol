pragma solidity >=0.8.9 <=0.8.24;

import {Subcall} from "@oasisprotocol/sapphire-contracts/contracts/Subcall.sol";

contract Oracle {
    // Maximum age of observations.
    uint private constant MAX_OBSERVATION_AGE = 10;

    // Configuration.
    uint8 public threshold;
    bytes21 public roflAppID;

    // Observations.
    struct Observation {
        uint128 value;
        uint block;
    }
    uint128[] private observations;
    Observation private lastObservation;

    constructor(bytes21 _roflAppID, uint8 _threshold) {
        require(_threshold > 0, "Invalid threshold");

        roflAppID = _roflAppID;
        threshold = _threshold;
        lastObservation.value = 0;
        lastObservation.block = 0;
    }

    function submitObservation(uint128 _value) external {
        // Ensure only the authorized ROFL app can submit.
        Subcall.roflEnsureAuthorizedOrigin(roflAppID);

        // NOTE: This is a naive oracle implementation for ROFL example purposes.
        // A real oracle must do additional checks and better aggregation before
        // accepting values.

        // Add observation and check if we have enough for this round.
        observations.push(_value);
        if (observations.length < threshold) {
            return;
        }

        // Simple averaging.
        uint256 _agg = 0;
        for (uint i = 0; i < observations.length; i++) {
            _agg += uint256(observations[i]);
        }
        _agg = _agg / uint128(observations.length);

        lastObservation.value = uint128(_agg);
        lastObservation.block = block.number;
        delete observations;
    }

    function getLastObservation() external view returns (uint128 _value, uint _block) {
        // Last observation must be fresh enough, otherwise we don't disclose it.
        require(
            lastObservation.block + MAX_OBSERVATION_AGE > block.number,
            "No observation available"
        );

        _value = lastObservation.value;
        _block = lastObservation.block;
    }
}
