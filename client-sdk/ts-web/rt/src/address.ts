import * as oasis from '@oasisprotocol/client';

import * as types from './types';

export const V0_SECP256K1_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: secp256k1';
export const V0_SECP256K1_CONTEXT_VERSION = 0;
export const V0_MULTISIG_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: multisig';
export const V0_MULTISIG_CONTEXT_VERSION = 0;

export async function fromSecp256k1PublicKey(pk: Uint8Array) {
    return await oasis.address.fromData(
        V0_SECP256K1_CONTEXT_IDENTIFIER,
        V0_SECP256K1_CONTEXT_VERSION,
        pk,
    );
}

export async function fromMultisigConfig(config: types.MultisigConfig) {
    const configU8 = oasis.misc.toCBOR(config);
    return await oasis.address.fromData(
        V0_MULTISIG_CONTEXT_IDENTIFIER,
        V0_MULTISIG_CONTEXT_VERSION,
        configU8,
    );
}
