// SPDX-License-Identifier: UNLICENSE
pragma solidity >=0.8.13;

import "tnt-core/BlueprintServiceManagerBase.sol";

/**
 * @title HelloBlueprint
 * @dev This contract is an example of a service blueprint that provides a single service.
 * @dev For all supported hooks, check the `BlueprintServiceManagerBase` contract.
 */
contract HelloBlueprint is BlueprintServiceManagerBase {
    /**
     * @dev Hook for service operator registration. Called when a service operator
     * attempts to register with the blueprint.
     * @param operator The operator's details.
     * @param registrationInputs Inputs required for registration in bytes format.
     */
    function onRegister(
        ServiceOperators.OperatorPreferences calldata operator,
        bytes calldata registrationInputs
    )
    external
    payable
    virtual
    override
    onlyFromMaster
    {
        // Do something with the operator's details
    }

    /**
     *  @dev Hook for service instance requests. Called when a user requests a service
     *  instance from the blueprint but this does not mean the service is initiated yet.
     *  To get notified when the service is initiated, implement the `onServiceInitialized` hook.
     *
     *  @param params The parameters for the service request.
     */
    function onRequest(ServiceOperators.RequestParams calldata params) external payable virtual override onlyFromMaster
    {
        // Do something with the service request
    }

    /**
     * @dev Hook for handling job result. Called when operators send the result
     * of a job execution.
     * @param serviceId The ID of the service related to the job.
     * @param job The job identifier.
     * @param jobCallId The unique ID for the job call.
     * @param operator The operator sending the result in bytes format.
     * @param inputs Inputs used for the job execution in bytes format.
     * @param outputs Outputs resulting from the job execution in bytes format.
     */
    function onJobResult(
        uint64 serviceId,
        uint8 job,
        uint64 jobCallId,
        ServiceOperators.OperatorPreferences calldata operator,
        bytes calldata inputs,
        bytes calldata outputs
    )
    external
    payable
    virtual
    override
    onlyFromMaster
    {
        // Do something with the job call result
    }

    /**
     * @dev Converts a public key to an operator address.
     * @param publicKey The public key to convert.
     * @return operator address The operator address.
     */
    function operatorAddressFromPublicKey(bytes calldata publicKey) internal pure returns (address operator) {
        return address(uint160(uint256(keccak256(publicKey))));
    }
}
