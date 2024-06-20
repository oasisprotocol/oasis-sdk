import {secp256k1} from '@noble/curves/secp256k1';
import {keccak_256} from '@noble/hashes/sha3';
import * as oasis from '@oasisprotocol/client';

import * as types from './types';

export const V0_SECP256K1ETH_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: secp256k1eth';
export const V0_SECP256K1ETH_CONTEXT_VERSION = 0;
export const V0_MULTISIG_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: multisig';
export const V0_MULTISIG_CONTEXT_VERSION = 0;

export function fromSigspec(spec: types.SignatureAddressSpec) {
    if (spec.ed25519) {
        return oasis.staking.addressFromPublicKey(spec.ed25519);
    } else if (spec.secp256k1eth) {
        // Use a scheme such that we can compute Secp256k1 addresses from Ethereum
        // addresses as this makes things more interoperable.
        const untaggedPk = secp256k1.ProjectivePoint.fromHex(spec.secp256k1eth)
            .toRawBytes(false)
            .slice(1);
        const pkData = keccak_256(new Uint8Array(untaggedPk)).slice(32 - 20);
        return oasis.address.fromData(
            V0_SECP256K1ETH_CONTEXT_IDENTIFIER,
            V0_SECP256K1ETH_CONTEXT_VERSION,
            pkData,
        );
    } else {
        throw new Error('unsupported signature address specification type');
    }
}

export function fromMultisigConfig(config: types.MultisigConfig) {
    const configU8 = oasis.misc.toCBOR(config);
    return oasis.address.fromData(
        V0_MULTISIG_CONTEXT_IDENTIFIER,
        V0_MULTISIG_CONTEXT_VERSION,
        configU8,
    );
}

export function toBech32(addr: Uint8Array) {
    return oasis.staking.addressToBech32(addr);
}

export function fromBech32(str: string) {
    return oasis.staking.addressFromBech32(str);
}
