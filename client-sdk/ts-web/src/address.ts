import * as bech32 from 'bech32';

const PREFIX = 'oasis';

export function toString(addr: Uint8Array) {
    return bech32.encode(PREFIX, bech32.toWords(addr));
}

export function fromString(str: string) {
    const {prefix, words} = bech32.decode(str);
    if (prefix !== PREFIX) throw new Error('wrong prefix: ' + prefix);
    return new Uint8Array(bech32.fromWords(words));
}
