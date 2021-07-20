import * as connection from './connection';
import * as protocol from './protocol';

/**
 * If the extension can share a list of keys and the web content wants to know
 * what keys are available, this method retrieves that list.
 */
export async function list(conn: connection.ExtConnection) {
    const req = {
        method: protocol.METHOD_KEYS_LIST,
    } as protocol.KeysListRequest;
    const res = (await conn.request(req)) as protocol.KeysListResponse;
    return res.keys;
}

/**
 * Register a callback to run when the list of available keys changes. Calls
 * to `list` that started before an invocation of this handler may return a
 * list that is out of date. A connection can have up to one of these
 * callbacks.
 */
export function setKeysChangeHandler(
    conn: connection.ExtConnection,
    handler: (event: protocol.KeysChangeEvent) => void,
) {
    conn.setEventHandler(protocol.EVENT_KEYS_CHANGE, handler);
}
