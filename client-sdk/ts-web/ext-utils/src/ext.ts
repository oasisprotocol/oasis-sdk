/**
 * @file The part to use from within an extension. (The rest of this package
 * is for use in web content.)
 *
 * There's logic here to accept connections from web content and pass requests
 * to your code and pass responses back to the web content.
 *
 * Run this code in a `web_accessible_resources` page named
 * `oasis-xu-frame.html`. Each web page creates its own instance of this page,
 * and this code communicates specifically with its parent web page.
 *
 * ```
 *                web app A
 *                    |
 *          +--------------------+
 *          |                    |
 *    page-a1.html         page-a2.html      } `oasisExt.connection` et al.
 *          |                    |
 * oasis-xu-frame.html  oasis-xu-frame.html  } `oasisExt.ext`
 *          |                    |
 *          +--------------------+
 *                    |
 *          extension shared state
 * ```
 */

import * as protocol from './protocol';

/**
 * A collection of methods that web content can access.
 */
export interface Handlers {
    /**
     * If the extension can share a list of keys and the web content wants to know
     * what keys are available, this method retrieves that list.
     * @param origin The origin where the request came from
     */
    keysList(origin: string, req: protocol.KeysListRequest): Promise<protocol.KeysListResponse>;
    /**
     * Get the public key of a ContextSigner kept by the extension.
     * @param origin The origin where the request came from
     */
    contextSignerPublic(
        origin: string,
        req: protocol.ContextSignerPublicRequest,
    ): Promise<protocol.ContextSignerPublicResponse>;
    /**
     * Sign a message with a ContextSigner kept by the extension.
     * @param origin The origin where the request came from
     */
    contextSignerSign(
        origin: string,
        req: protocol.ContextSignerSignRequest,
    ): Promise<protocol.ContextSignerSignResponse>;
}

/**
 * Call this to let the web content start making requests.
 */
export function ready(handlers: Handlers) {
    window.addEventListener('message', async (e: MessageEvent<unknown>) => {
        // @ts-expect-error even if .type is missing, it's fine if we get undefined here
        const messageType = e.data.type;
        switch (messageType) {
            case protocol.MESSAGE_TYPE_REQUEST: {
                const reqM = e.data as protocol.MessageRequest;
                const resM = {
                    type: protocol.MESSAGE_TYPE_RESPONSE,
                    id: reqM.id,
                } as protocol.MessageResponse;
                try {
                    // @ts-expect-error even if .method is missing, it's fine if we get undefined here
                    const method = reqM.body.method;
                    switch (method) {
                        case protocol.METHOD_KEYS_LIST: {
                            const req = reqM.body as protocol.KeysListRequest;
                            resM.body = await handlers.keysList(e.origin, req);
                            break;
                        }
                        case protocol.METHOD_CONTEXT_SIGNER_PUBLIC: {
                            const req = reqM.body as protocol.ContextSignerPublicRequest;
                            resM.body = await handlers.contextSignerPublic(e.origin, req);
                            break;
                        }
                        case protocol.METHOD_CONTEXT_SIGNER_SIGN: {
                            const req = reqM.body as protocol.ContextSignerSignRequest;
                            if (typeof req.context !== 'string') {
                                throw new Error(`${method}: .context must be string`);
                            }
                            if (!(req.message instanceof Uint8Array)) {
                                throw new Error(`${method}: .message must be Uint8Array`);
                            }
                            resM.body = await handlers.contextSignerSign(e.origin, req);
                            break;
                        }
                        default: {
                            throw new Error(`unhandled method ${method}`);
                        }
                    }
                } catch (e) {
                    resM.err = e;
                } finally {
                    (e.source as WindowProxy).postMessage(resM, e.origin);
                }
                break;
            }
        }
    });
    window.parent.postMessage(
        {
            type: protocol.MESSAGE_TYPE_READY,
        } as protocol.MessageReady,
        '*',
    );
}

function postEvent(event: unknown) {
    window.parent.postMessage(
        {
            type: protocol.MESSAGE_TYPE_EVENT,
            event: event,
        } as protocol.MessageEvent,
        '*',
    );
}

/**
 * Call this to tell the web content that the list of available keys has changed.
 * @param keys The new list of available keys, as would be returned from `keysList`
 */
export function keysChanged(keys: protocol.KeyInfo[]) {
    postEvent({
        type: protocol.EVENT_KEYS_CHANGE,
        keys,
    } as protocol.KeysChangeEvent);
}
