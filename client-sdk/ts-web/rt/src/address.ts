import * as oasis from '@oasisprotocol/client';
import * as elliptic from 'elliptic';
import {Keccak} from 'sha3';

import * as types from './types';

export const V0_SECP256K1ETH_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: secp256k1eth';
export const V0_SECP256K1ETH_CONTEXT_VERSION = 0;
export const V0_MULTISIG_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: multisig';
export const V0_MULTISIG_CONTEXT_VERSION = 0;

const SECP256K1 = new elliptic.ec('secp256k1');

export async function fromSigspec(spec: types.SignatureAddressSpec) {
    if (spec.ed25519) {
        return await oasis.staking.addressFromPublicKey(spec.ed25519);
    } else if (spec.secp256k1eth) {
        // Use a scheme such that we can compute Secp256k1 addresses from Ethereum
        // addresses as this makes things more interoperable.
        const untaggedPk = SECP256K1.keyFromPublic(Array.from(spec.secp256k1eth))
            .getPublic(false, 'array')
            .slice(1);
        const hash = new Keccak(256);
        hash.update(Buffer.from(untaggedPk));
        const pkData = hash.digest().slice(32 - 20);
        return await oasis.address.fromData(
            V0_SECP256K1ETH_CONTEXT_IDENTIFIER,
            V0_SECP256K1ETH_CONTEXT_VERSION,
            pkData,
        );
    } else {
        throw new Error('unsupported signature address specification type');
    }
}

export async function fromMultisigConfig(config: types.MultisigConfig) {
    const configU8 = oasis.misc.toCBOR(config);
    return await oasis.address.fromData(
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
