import {sha512_256} from '@noble/hashes/sha512';

export async function hash(data: Uint8Array) {
    return sha512_256(data);
}
