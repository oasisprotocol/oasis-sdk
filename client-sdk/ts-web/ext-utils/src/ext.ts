import * as protocol from './protocol';

export interface Handlers {
    contextSignerPublic(
        origin: string,
        req: protocol.ContextSignerPublicRequest,
    ): Promise<protocol.ContextSignerPublicResponse>;
    contextSignerSign(
        origin: string,
        req: protocol.ContextSignerSignRequest,
    ): Promise<protocol.ContextSignerSignResponse>;
}

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
                        case protocol.METHOD_CONTEXT_SIGNER_PUBLIC: {
                            const req = reqM.body as protocol.ContextSignerPublicRequest;
                            resM.body = await handlers.contextSignerPublic(e.origin, req);
                            break;
                        }
                        case protocol.METHOD_CONTEXT_SIGNER_SIGN: {
                            const req = reqM.body as protocol.ContextSignerSignRequest;
                            if (typeof req.context !== 'string')
                                throw new Error(`${method}: .context must be string`);
                            if (!(req.message instanceof Uint8Array))
                                throw new Error(`${method}: .message must be Uint8Array`);
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
