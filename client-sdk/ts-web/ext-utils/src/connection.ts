/**
 * @file Messaging between web content and extension.
 *
 * For this, we use a 'web_accessible_resource' page that a web page can embed
 * in an iframe. The parent content frame and the embedded extension frame can
 * then `postMessage` with each other.
 */

import * as protocol from './protocol';

let addedMessageListener = false;
const connectionsPromised: {[origin: string]: Promise<ExtConnection>} = {};
const connectionsRequested: {[origin: string]: {resolve: any; reject: any}} = {};
const responseHandlers: {[handlerKey: string]: {resolve: any; reject: any}} = {};
const eventHandlers: {[handlerKey: string]: (event: never) => void} = {};

/**
 * A communication channel with an extension.
 *
 * It supports a basic request-response kind of interaction (web content is
 * the requester). The meaning of the requests and responses are defined in
 * another layer of abstraction.
 *
 * Use `create` to create one.
 */
export class ExtConnection {
    origin: string;
    messageFrame: WindowProxy;
    nextId: number;

    constructor(origin: string, messageFrame: WindowProxy) {
        this.origin = origin;
        this.messageFrame = messageFrame;
        this.nextId = 0;
    }

    request(req: unknown) {
        return new Promise((resolve, reject) => {
            const reqId = this.nextId++;
            const handlerKey = `${this.origin}/${reqId}`;
            responseHandlers[handlerKey] = {resolve, reject};
            this.messageFrame.postMessage(
                {
                    type: protocol.MESSAGE_TYPE_REQUEST,
                    id: reqId,
                    body: req,
                } as protocol.MessageRequest,
                this.origin,
            );
        });
    }

    setEventHandler(type: string, handler: (event: never) => void) {
        const eventKey = `${this.origin}/${type}`;
        eventHandlers[eventKey] = handler;
    }
}

export function handleMessage(e: MessageEvent<unknown>) {
    // @ts-expect-error even if .type is missing, it's fine if we get undefined here
    const messageType = e.data.type;
    switch (messageType) {
        case protocol.MESSAGE_TYPE_READY: {
            const m = e.data as protocol.MessageReady;
            if (!connectionsRequested[e.origin]) break;
            const {resolve, reject} = connectionsRequested[e.origin];
            delete connectionsRequested[e.origin];
            const connection = new ExtConnection(e.origin, e.source as WindowProxy);
            resolve(connection);
            break;
        }
        case protocol.MESSAGE_TYPE_RESPONSE: {
            const m = e.data as protocol.MessageResponse;
            const handlerKey = `${e.origin}/${m.id}`;
            if (!responseHandlers[handlerKey]) break;
            const {resolve, reject} = responseHandlers[handlerKey];
            delete responseHandlers[handlerKey];
            if ('err' in m) {
                reject(m.err);
            } else {
                resolve(m.body);
            }
            break;
        }
        case protocol.MESSAGE_TYPE_EVENT: {
            const m = e.data as protocol.MessageEvent;
            // @ts-expect-error if m.event.type is missing and we get undefined, we'll survive
            const handlerKey = `${e.origin}/${m.event.type}`;
            if (!eventHandlers[handlerKey]) break;
            const handler = eventHandlers[handlerKey];
            handler(m.event as never);
            break;
        }
    }
}

/**
 * Set up a connection with an extension, identified by its origin. This
 * includes adding an iframe to the document. This requires `document.body`
 * to exist.
 *
 * Gives a promise, so await the result. The promise will hang if the user
 * doesn't have the extension installed.
 *
 * This module keeps an inventory of connections that it has already set up,
 * and it'll give you the the connection promise that it already has if it has
 * one.
 *
 * The connection stays open, and there is no disconnect.
 *
 * @param origin This will look like `chrome-extension://xxxxxxxxxxxxxxxxxx`
 * @param path The path plus the origin will be the URL we use to load the
 * iframe. Default is `/oasis-xu-frame.html`
 */
export function connect(origin: string, path = '/oasis-xu-frame.html') {
    if (!addedMessageListener) {
        window.addEventListener('message', handleMessage);
        addedMessageListener = true;
    }
    if (!connectionsPromised[origin]) {
        connectionsPromised[origin] = new Promise((resolve, reject) => {
            connectionsRequested[origin] = {resolve, reject};
        });

        const iframe = document.createElement('iframe');
        iframe.src = `${origin}${path}`;
        iframe.hidden = true;
        document.body.appendChild(iframe);
    }
    return connectionsPromised[origin];
}
