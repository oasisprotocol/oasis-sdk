import {bech32} from 'bech32';

import * as hash from './hash';
import * as misc from './misc';

export function fromData(contextIdentifier: string, contextVersion: number, data: Uint8Array) {
    const versionU8 = new Uint8Array([contextVersion]);
    return misc.concat(
        versionU8,
        hash.hash(misc.concat(misc.fromString(contextIdentifier), versionU8, data)).slice(0, 20),
    );
}

export function toBech32(prefix: string, addr: Uint8Array) {
    return bech32.encode(prefix, bech32.toWords(addr));
}

export function fromBech32(expectedPrefix: string, str: string) {
    const {prefix, words} = bech32.decode(str);
    if (prefix !== expectedPrefix) {
        throw new Error(`wrong prefix: ${prefix}, expected ${expectedPrefix}`);
    }
    return new Uint8Array(bech32.fromWords(words));
}
