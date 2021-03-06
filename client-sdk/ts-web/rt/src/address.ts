import * as oasis from '@oasisprotocol/client';

export const V0_SECP256K1_CONTEXT_IDENTIFIER = 'oasis-runtime-sdk/address: secp256k1';
export const V0_SECP256K1_CONTEXT_VERSION = 0;

export async function fromSecp256k1PublicKey(pk: Uint8Array) {
    return await oasis.address.fromData(
        V0_SECP256K1_CONTEXT_IDENTIFIER,
        V0_SECP256K1_CONTEXT_VERSION,
        pk,
    );
}
