import * as bech32 from 'bech32';

import * as hash from './hash';
import * as misc from './misc';

const PREFIX = 'oasis';

export async function fromPublicKey(contextIdentifier: string, contextVersion: number, pk: Uint8Array) {
    const versionU8 = new Uint8Array([contextVersion]);
    return misc.concat(versionU8, (await hash.hash(misc.concat(misc.fromString(contextIdentifier), versionU8, pk))).slice(0, 20));
}

export function toString(addr: Uint8Array) {
    return bech32.encode(PREFIX, bech32.toWords(addr));
}

export function fromString(str: string) {
    const {prefix, words} = bech32.decode(str);
    if (prefix !== PREFIX) throw new Error('wrong prefix: ' + prefix);
    return new Uint8Array(bech32.fromWords(words));
}
