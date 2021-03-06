/** Returns the SHA-512/256 hash of `data`. */
export async function hash(data: Uint8Array) {
    const dataBuf = data.buffer.slice(
        data.byteOffset,
        data.byteOffset + data.byteLength
    );
    const hashBuf = await crypto.subtle.digest("SHA-512", dataBuf);
    return new Uint8Array(hashBuf, 0, 256 / 8);
}
