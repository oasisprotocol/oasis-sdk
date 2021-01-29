// @ts-expect-error missing declaration
import * as cborg from 'cborg';

const HEX_DIGITS = '0123456789abcdef';

// oasis-core routinely uses maps with non-string keys, e.g. in a staking Delegations response. We
// can't pick and choose which CBOR maps to decode into objects and which to decode in to Maps, so
// walk through the data after decoding and find any string-keys-only and recreate them as objects.
function objsFromMaps(v: any): any {
    if (v instanceof Map) {
        let keysCompatible = true;
        for (const key of v.keys()) {
            if (typeof key !== 'string') {
                keysCompatible = false;
                break;
            }
        }
        if (v.size > 0 && keysCompatible) {
            // Recreate as an object.
            const o: {[key: string]: any} = {};
            for (const [key, val] of v) {
                o[key] = objsFromMaps(val);
            }
            return o;
        } else {
            // Leave as a Map. We'd miss empty structs, but we wouldn't dare use such a thing,
            // would we?
            const m = new Map();
            for (const [key, val] of v) {
                m.set(objsFromMaps(key), objsFromMaps(val));
            }
            return m;
        }
    } else if (v instanceof Array) {
        const a = [];
        for (const elem of v) {
            a.push(objsFromMaps(elem));
        }
        return a;
    } else {
        return v;
    }
}

export function toCBOR(v: any): Uint8Array {
    return cborg.encode(v);
}

export function fromCBOR(u8: Uint8Array) {
    // oasis-core uses a special case to marshal `nil` to an empty byte string.
    if (!u8.length) return undefined;
    return objsFromMaps(cborg.decode(u8, {useMaps: true}));
}

export function toHex(u8: Uint8Array) {
    let hex = '';
    for (const b of u8) {
        hex += HEX_DIGITS[b >>> 4] + HEX_DIGITS[b & 0xf];
    }
    return hex;
}

export function fromHex(hex: string) {
    let byteLength = hex.length >> 1;
    const u8 = new Uint8Array(byteLength);
    for (let i = 0; i < byteLength; i++) {
        u8[i] = parseInt(hex.substr(2 * i, 2), 16);
    }
    return u8;
}

export function fromString(str: string) {
    return new TextEncoder().encode(str);
}

export function concat(...parts: Uint8Array[]) {
    let length = 0;
    for (const part of parts) {
        length += part.length;
    }
    let result = new Uint8Array(length);
    let pos = 0;
    for (const part of parts) {
        result.set(part, pos);
        pos += part.length;
    }
    return result;
}
