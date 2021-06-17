import * as connection from './connection';
import * as protocol from './protocol';

/**
 * If the extension can share a list of keys and the web content wants to know
 * what keys are available, this method retrieves that list.
 */
export async function list(connection: connection.ExtConnection) {
    const req = {
        method: protocol.METHOD_KEYS_LIST,
    } as protocol.KeysListRequest;
    const res = (await connection.request(req)) as protocol.KeysListResponse;
    return res.keys;
}
