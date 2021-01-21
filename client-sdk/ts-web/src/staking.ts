import * as address from './address';

const CONTEXT_IDENTIFIER = 'oasis-core/address: staking';
const CONTEXT_VERSION = 0;

export async function addressFromPublicKey(pk: Uint8Array) {
    return await address.fromPublicKey(CONTEXT_IDENTIFIER, CONTEXT_VERSION, pk);
}
