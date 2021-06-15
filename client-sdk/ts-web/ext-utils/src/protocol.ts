// page <- message frame
export const MESSAGE_TYPE_READY = 'oasis-xu-ready';
export const MESSAGE_TYPE_RESPONSE = 'oasis-xu-response';

export interface MessageReady {
    type: typeof MESSAGE_TYPE_READY;
}

export interface MessageResponse {
    type: typeof MESSAGE_TYPE_RESPONSE;
    id: number;
    body?: unknown;
    err?: unknown;
}

export interface ContextSignerPublicResponse {
    public_key: Uint8Array;
}

export interface ContextSignerSignResponse {
    approved: boolean;
    signature?: Uint8Array;
}

// page -> message frame
export const MESSAGE_TYPE_REQUEST = 'oasis-xu-request';

export interface MessageRequest {
    type: typeof MESSAGE_TYPE_REQUEST;
    id: number;
    body: unknown;
}

export const METHOD_CONTEXT_SIGNER_PUBLIC = 'context-signer-public';
export const METHOD_CONTEXT_SIGNER_SIGN = 'context-signer-sign';

export interface ContextSignerPublicRequest {
    method: typeof METHOD_CONTEXT_SIGNER_PUBLIC;
    which: any;
}

export interface ContextSignerSignRequest {
    method: typeof METHOD_CONTEXT_SIGNER_SIGN;
    which: any;
    context: string;
    message: Uint8Array;
}
