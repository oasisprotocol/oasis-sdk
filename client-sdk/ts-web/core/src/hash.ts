import {sha512_256} from 'js-sha512';

export async function hash(data: Uint8Array) {
    return new Uint8Array(sha512_256.arrayBuffer(data));
}
