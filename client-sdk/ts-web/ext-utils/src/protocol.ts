/**
 * @file Constants and type definitions for the `postMessage`-based protocol
 * used between web content and the extension.
 */

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

// page -> message frame
export const MESSAGE_TYPE_REQUEST = 'oasis-xu-request';

export interface MessageRequest {
    type: typeof MESSAGE_TYPE_REQUEST;
    id: number;
    body: unknown;
}

// methods

export const METHOD_CONTEXT_SIGNER_PUBLIC = 'context-signer-public';

export interface ContextSignerPublicRequest {
    method: typeof METHOD_CONTEXT_SIGNER_PUBLIC;
    /**
     * An extra parameter, suggested to be used to help select which key to
     * use. This library passes it through verbatim; it's up to the extension
     * to define how it's interpreted.
     */
    which: any;
}

export interface ContextSignerPublicResponse {
    public_key: Uint8Array;
}

export const METHOD_CONTEXT_SIGNER_SIGN = 'context-signer-sign';

export interface ContextSignerSignRequest {
    method: typeof METHOD_CONTEXT_SIGNER_SIGN;
    /**
     * An extra parameter, suggested to be used to help select which key to
     * use. This library passes it through verbatim; it's up to the extension
     * to define how it's interpreted.
     */
    which: any;
    context: string;
    message: Uint8Array;
}

/**
 * Either the signature as requested (and with `approved: true`) or an
 * explicit decline to sign (`approved: false` and no `signature`).
 */
export interface ContextSignerSignResponse {
    approved: boolean;
    signature?: Uint8Array;
}
