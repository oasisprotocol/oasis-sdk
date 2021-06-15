import * as protocol from './protocol';

let addedMessageListener = false;
const connectionsPromised: {[origin: string]: Promise<ExtConnection>} = {};
const connectionsRequested: {[origin: string]: {resolve: any; reject: any}} = {};
const responseHandlers: {[handlerKey: string]: {resolve: any; reject: any}} = {};

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
}

export function handleMessage(e: MessageEvent<unknown>) {
    // @ts-expect-error even if .type is missing, it's fine if we get undefined here
    const messageType = e.data.type;
    switch (messageType) {
        case protocol.MESSAGE_TYPE_READY: {
            const m = e.data as protocol.MessageReady;
            if (!(e.origin in connectionsRequested)) break;
            const {resolve, reject} = connectionsRequested[e.origin];
            const connection = new ExtConnection(e.origin, e.source as WindowProxy);
            resolve(connection);
            break;
        }
        case protocol.MESSAGE_TYPE_RESPONSE: {
            const m = e.data as protocol.MessageResponse;
            const handlerKey = `${e.origin}/${m.id}`;
            if (!(handlerKey in responseHandlers)) break;
            const {resolve, reject} = responseHandlers[handlerKey];
            delete responseHandlers[handlerKey];
            if ('err' in m) {
                reject(m.err);
            } else {
                resolve(m.body);
            }
            break;
        }
    }
}

export function connect(origin: string) {
    if (!addedMessageListener) {
        window.addEventListener('message', handleMessage);
        addedMessageListener = true;
    }
    if (!(origin in connectionsPromised)) {
        connectionsPromised[origin] = new Promise((resolve, reject) => {
            connectionsRequested[origin] = {resolve, reject};
        });

        const iframe = document.createElement('iframe');
        iframe.src = `${origin}/oasis-xu-frame.html`;
        iframe.hidden = true;
        document.body.appendChild(iframe);
    }
    return connectionsPromised[origin];
}
