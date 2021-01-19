import * as bech32 from 'bech32';

const PREFIX = 'oasis';

export function strFromU8(u8: Uint8Array) {
    return bech32.encode(PREFIX, bech32.toWords(u8));
}

export function u8FromStr(str: string) {
    const {prefix, words} = bech32.decode(str);
    if (prefix !== PREFIX) throw new Error('wrong prefix: ' + prefix);
    return new Uint8Array(bech32.fromWords(words));
}
