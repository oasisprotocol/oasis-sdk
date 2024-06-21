import {sha512_256} from '@noble/hashes/sha512';

export function hash(data: Uint8Array) {
    return sha512_256(data);
}
