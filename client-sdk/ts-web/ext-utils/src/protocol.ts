/**
 * @file Constants and type definitions for the `postMessage`-based protocol
 * used between web content and the extension.
 */

// page <- message frame
export const MESSAGE_TYPE_READY = 'oasis-xu-ready-v1';
export const MESSAGE_TYPE_RESPONSE = 'oasis-xu-response-v1';
export const MESSAGE_TYPE_EVENT = 'oasis-xu-event-v1';

export interface MessageReady {
    type: typeof MESSAGE_TYPE_READY;
}

export interface MessageResponse {
    type: typeof MESSAGE_TYPE_RESPONSE;
    id: number;
    body?: unknown;
    err?: unknown;
}

export interface MessageEvent {
    type: typeof MESSAGE_TYPE_EVENT;
    event: unknown;
}

// page -> message frame
export const MESSAGE_TYPE_REQUEST = 'oasis-xu-request-v1';

export interface MessageRequest {
    type: typeof MESSAGE_TYPE_REQUEST;
    id: number;
    body: unknown;
}

// methods

export const METHOD_KEYS_LIST = 'keys-list-v1';

export interface KeysListRequest {
    method: typeof METHOD_KEYS_LIST;
}

export interface KeyInfo {
    /**
     * A value for the `which` parameter when requesting a public key or a
     * signature. It's up to the extension to define how it's structured.
     */
    which: any;
    /**
     * An extra output, suggested to be used to describe the key. It's up to
     * the extension to define how it's structured.
     */
    metadata: any;
}

export interface KeysListResponse {
    keys: KeyInfo[];
}

export const METHOD_CONTEXT_SIGNER_PUBLIC = 'context-signer-public-v1';

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

export const METHOD_CONTEXT_SIGNER_SIGN = 'context-signer-sign-v1';

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

// events

export const EVENT_KEYS_CHANGE = 'keys-change-v1';

export interface KeysChangeEvent {
    type: typeof EVENT_KEYS_CHANGE;
    /**
     * The new list of available keys, as would be returned from `keys.list`.
     */
    keys: KeyInfo[];
}
