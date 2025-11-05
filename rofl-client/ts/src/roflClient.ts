import * as http from 'node:http';
import * as https from 'node:https';
import {readFileSync} from 'node:fs';
import {resolve as resolvePath} from 'node:path';

/** Default Unix domain socket path. */
export const ROFL_SOCKET_PATH = '/run/rofl-appd.sock';

/** Supported key generation types for ROFL.
 *
 * - `RAW_256`: Generate 256 bits of entropy.
 * - `RAW_384`: Generate 384 bits of entropy.
 * - `ED25519`: Generate an Ed25519 private key.
 * - `SECP256K1`: Generate a Secp256k1 private key.
 *
 * Values serialize to the stable strings expected by `/rofl/v1/keys/generate`.
 */
export enum KeyKind {
    RAW_256 = 'raw-256',
    RAW_384 = 'raw-384',
    ED25519 = 'ed25519',
    SECP256K1 = 'secp256k1',
}

/** Client options. */
export interface RoflClientOptions {
    /** Base URL or UDS path. */
    url?: string;
    /**
     * Request timeout in milliseconds. Default: 60_000 (socket inactivity timeout).
     * Note: This bounds socket inactivity, not absolute wall-clock time.
     */
    timeoutMs?: number;
    /** Optional User-Agent header (defaults to '@oasisprotocol/rofl-client/<version>'). */
    userAgent?: string;
    /** Internal: pluggable transport (used for testing). */
    transport?: Transport;
}

/** Internal transport request/response types (simple and testable). */
export interface TransportRequest {
    method: 'GET' | 'POST';
    path: string;
    /** If HTTP(S) is used. */
    baseUrl?: string;
    /** If UDS is used. */
    socketPath?: string;
    payload?: unknown;
    /** Serialized payload body if already encoded. */
    body?: string;
    timeoutMs: number;
    headers?: Record<string, string>;
}

export interface TransportResponse {
    status: number;
    /** Parsed JSON if content-type is JSON, otherwise raw string. */
    body: unknown;
    headers: Record<string, string | string[] | undefined>;
}

export type Transport = (req: TransportRequest) => Promise<TransportResponse>;

export type StdTx = {
    /** Kind marker for standard Oasis SDK transactions. */
    kind: 'std';
    /** CBOR-serialized hex-encoded Transaction bytes (with or without 0x). */
    data: string;
};

export type EthTx = {
    /** Kind marker for Ethereum-compatible calls. */
    kind: 'eth';
    /** Gas limit to include in the transaction. */
    gas_limit: number;
    /**
     * Hex-encoded destination address (with or without 0x). Empty string indicates contract creation.
     * The backend expects raw bytes parsed from hex.
     */
    to: string;
    /**
     * Transaction value. NOTE: This is a JSON number and must fit in JS number range.
     * The backend expects a `u128`, but does not currently accept strings.
     */
    value: number;
    /** Hex-encoded calldata (with or without 0x). */
    data: string;
};

type AdjacentStdTx = {kind: 'std'; data: string};
type AdjacentEthTx = {
    kind: 'eth';
    data: {gas_limit: number; to: string; value: number; data: string};
};

type TxPayload = {tx: AdjacentStdTx | AdjacentEthTx; encrypt: boolean};

function stripHexPrefix(h: string): string {
    if (!h) return '';
    return h.startsWith('0x') || h.startsWith('0X') ? h.slice(2) : h;
}

function normalizeTx(tx: StdTx | EthTx): AdjacentStdTx | AdjacentEthTx {
    if (tx.kind === 'std') {
        return {kind: 'std', data: stripHexPrefix(tx.data)};
    }
    return {
        kind: 'eth',
        data: {
            gas_limit: tx.gas_limit,
            to: stripHexPrefix(tx.to),
            value: tx.value,
            data: stripHexPrefix(tx.data),
        },
    };
}

function hexToBytes(hex: string): Uint8Array {
    const h = stripHexPrefix(hex);
    if (h.length % 2 !== 0) throw new Error('Invalid hex string length');
    // Buffer is a Uint8Array subclass in Node.
    return Buffer.from(h, 'hex');
}

const PACKAGE_VERSION = (() => {
    try {
        const pkgPath = resolvePath(__dirname, '..', 'package.json');
        const raw = readFileSync(pkgPath, 'utf8');
        const parsed = JSON.parse(raw) as {version?: string};
        return typeof parsed.version === 'string' ? parsed.version : 'dev';
    } catch {
        return 'dev';
    }
})();

const DEFAULT_USER_AGENT = `@oasisprotocol/rofl-client/${PACKAGE_VERSION}`;

/**
 * Client for interacting with the ROFL application daemon REST API.
 *
 * Provides methods for key generation, metadata management, and authenticated
 * transaction submission to ROFL applications running in trusted execution environments.
 *
 * @example
 * ```typescript
 * // Connect via Unix Domain Socket (default)
 * const client = new RoflClient();
 *
 * // Connect via HTTP
 * const httpClient = new RoflClient({ url: 'http://localhost:8080' });
 *
 * // Custom socket path with timeout
 * const customClient = new RoflClient({
 *   url: '/custom/rofl.sock',
 *   timeoutMs: 30000
 * });
 * ```
 */
export class RoflClient {
    private readonly url: string;
    private readonly timeoutMs: number;
    private readonly transport: Transport;
    private readonly userAgent: string;

    constructor(opts: RoflClientOptions = {}) {
        this.url = opts.url ?? '';
        this.timeoutMs = opts.timeoutMs ?? 60_000;
        this.transport = opts.transport ?? nodeTransport;
        this.userAgent = opts.userAgent ?? DEFAULT_USER_AGENT;
    }

    /**
     * Generate or fetch a cryptographic key from ROFL's decentralized key management system.
     *
     * All generated keys are deterministic and tied to the app's identity. They can only be
     * generated inside properly attested app instances and will remain consistent across
     * deployments or state resets.
     *
     * @param keyId - Domain separator for different keys within the application (e.g., 'signing-key', 'encryption-key')
     * @param kind - Type of key to generate (default: SECP256K1). See {@link KeyKind} for available options
     * @returns Hex-encoded key material without 0x prefix. For cryptographic keys (ED25519, SECP256K1),
     *          returns the private key. For entropy types (RAW_256, RAW_384), returns raw random bytes
     * @throws {Error} If key generation fails or response is invalid
     *
     * @example
     * ```typescript
     * // Generate a default SECP256K1 key
     * const key = await client.generateKey('my-signing-key');
     *
     * // Generate an Ed25519 key for a specific purpose
     * const ed25519Key = await client.generateKey('my-ed25519-key', KeyKind.ED25519);
     *
     * // Generate raw entropy for custom use
     * const entropy = await client.generateKey('my-entropy', KeyKind.RAW_256);
     * ```
     */
    async generateKey(keyId: string, kind: KeyKind = KeyKind.SECP256K1): Promise<string> {
        const res = await this.appdRequest('POST', '/rofl/v1/keys/generate', {
            key_id: keyId,
            kind,
        });

        if (
            !res ||
            typeof res !== 'object' ||
            res === null ||
            typeof (res as Record<string, unknown>).key !== 'string'
        ) {
            throw new Error('Invalid response from ROFL key generation');
        }

        return (res as {key: string}).key;
    }

    /**
     * Retrieve all user-set metadata key-value pairs for this ROFL app instance.
     *
     * Metadata is automatically namespaced with 'net.oasis.app.' when published in the
     * on-chain ROFL replica registration, but this method returns the raw keys without
     * the namespace prefix.
     *
     * @returns Dictionary of metadata key-value pairs
     * @throws {Error} If metadata retrieval fails or response is invalid
     *
     * @example
     * ```typescript
     * const metadata = await client.getMetadata();
     * console.log(metadata);
     * // Output: { "version": "1.0.0", "key_fingerprint": "abc123..." }
     * ```
     */
    async getMetadata(): Promise<Record<string, string>> {
        const res = await this.appdRequest('GET', '/rofl/v1/metadata');

        if (!res || typeof res !== 'object' || res === null) {
            throw new Error('Invalid response from ROFL metadata');
        }

        return res as Record<string, string>;
    }

    /**
     * Set metadata key-value pairs for this ROFL app instance.
     *
     * This replaces **all** existing app-provided metadata. If the metadata has changed,
     * it will trigger an automatic on-chain registration refresh. Keys are automatically
     * namespaced with 'net.oasis.app.' when published on-chain.
     *
     * Metadata is validated against runtime-configured limits (typically max 64 pairs,
     * max key size 1024 bytes, max value size 16KB).
     *
     * @param metadata - Dictionary of metadata key-value pairs to set
     * @returns Promise that resolves when metadata is successfully updated
     * @throws {Error} If metadata validation fails or update is rejected
     *
     * @example
     * ```typescript
     * // Publish app version and key fingerprint
     * await client.setMetadata({
     *   version: '1.0.0',
     *   key_fingerprint: 'abc123...'
     * });
     *
     * // Clear all metadata by setting empty object
     * await client.setMetadata({});
     * ```
     */
    async setMetadata(metadata: Record<string, string>): Promise<void> {
        await this.appdRequest('POST', '/rofl/v1/metadata', metadata);
    }

    /**
     * Retrieve the ROFL app identifier in bech32 format.
     *
     * The app ID uniquely identifies this ROFL application on-chain and is used
     * for authentication and authorization. This is a convenience method that wraps
     * the `/rofl/v1/app/id` endpoint.
     *
     * @returns Bech32-encoded app ID (e.g., 'rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf')
     * @throws {Error} If app ID retrieval fails or response is invalid
     *
     * @example
     * ```typescript
     * const appId = await client.getAppId();
     * console.log(`My ROFL app ID: ${appId}`);
     * // Output: "My ROFL app ID: rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf"
     * ```
     */
    async getAppId(): Promise<string> {
        const res = await this.appdRequest('GET', '/rofl/v1/app/id');

        if (typeof res !== 'string') {
            throw new Error('Invalid response while fetching ROFL app id');
        }

        return res;
    }

    /** Sign and submit an authenticated transaction via ROFL.
     *
     * @param tx - A `StdTx` or `EthTx`. Hex fields may include a `0x` prefix; it will be stripped.
     * @param opts.encrypt - Whether to encrypt calldata. Defaults to `true` (server default).
     * @returns Raw CBOR-encoded `CallResult` bytes returned by the runtime.
     */
    async signAndSubmit(tx: StdTx | EthTx, opts?: {encrypt?: boolean}): Promise<Uint8Array> {
        const payload: TxPayload = {
            tx: normalizeTx(tx),
            encrypt: opts?.encrypt ?? true,
        };
        const res = await this.appdRequest('POST', '/rofl/v1/tx/sign-submit', payload);

        if (!res || typeof res !== 'object' || res === null) {
            throw new Error('Invalid response from ROFL sign-submit');
        }
        const data = (res as Record<string, unknown>).data;
        if (typeof data !== 'string') {
            throw new Error('Invalid response: missing hex-encoded call result');
        }
        return hexToBytes(data);
    }

    // internals

    private isHttpUrl(): boolean {
        return this.url.startsWith('http://') || this.url.startsWith('https://');
    }

    private socketPathOrDefault(): string {
        if (this.url && !this.isHttpUrl()) return this.url;
        if (!this.url) return ROFL_SOCKET_PATH;
        return '';
    }

    private async appdRequest(
        method: 'GET' | 'POST',
        path: string,
        payload?: unknown,
    ): Promise<unknown> {
        const headers: Record<string, string> = {
            'User-Agent': this.userAgent,
        };
        let body: string | undefined;

        if (method === 'POST') {
            body = JSON.stringify(payload ?? {});
            // Canonical header casing for broader compatibility.
            headers['Content-Type'] = 'application/json';
            headers['Content-Length'] = Buffer.byteLength(body).toString();
        }

        const transportRequest: TransportRequest = this.isHttpUrl()
            ? {
                  method,
                  path,
                  baseUrl: this.url,
                  payload,
                  body,
                  timeoutMs: this.timeoutMs,
                  headers,
              }
            : {
                  method,
                  path,
                  socketPath: this.socketPathOrDefault(),
                  payload,
                  body,
                  timeoutMs: this.timeoutMs,
                  headers,
              };

        const {status, body: responseBody} = await this.transport(transportRequest);

        if (status >= 400) {
            const message =
                typeof responseBody === 'string' ? responseBody : JSON.stringify(responseBody);
            throw new Error(`ROFL request failed (${status}): ${message}`);
        }

        return responseBody;
    }
}

/** Default Node.js transport with UDS + HTTP(S) support. */
const nodeTransport: Transport = (req) =>
    new Promise<TransportResponse>((resolve, reject) => {
        // Note: Unix Domain Sockets don't use TLS - they're local IPC mechanisms
        // where security comes from filesystem permissions, not encryption.
        // Only network connections (HTTP/HTTPS) need to consider TLS.
        const isHttp = !!req.baseUrl;
        const isHttps = isHttp && req.baseUrl!.startsWith('https://');
        const lib = isHttp ? (isHttps ? https : http) : http;

        const onError = (err: Error) => reject(err);

        const writeBody = (request: http.ClientRequest) => {
            if (req.method === 'POST') {
                const payload = req.body ?? JSON.stringify(req.payload ?? {});
                request.write(payload);
            }
        };

        const attachTimeout = (request: http.ClientRequest) => {
            request.setTimeout(req.timeoutMs, () => {
                request.destroy(new Error('ROFL request timed out'));
            });
        };

        const onResponse = (res: http.IncomingMessage) => {
            const chunks: Buffer[] = [];

            res.on('data', (chunk) => {
                chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
            });

            res.on('end', () => {
                const raw = Buffer.concat(chunks);
                const text = raw.toString('utf8');
                const contentType = (res.headers['content-type'] || '').toString();

                let response: unknown = text;
                if (text.length === 0) {
                    response = '';
                }
                if (contentType.includes('application/json')) {
                    try {
                        response = text.length ? JSON.parse(text) : {};
                    } catch (e) {
                        reject(new Error(`Invalid JSON in ROFL response: ${(e as Error).message}`));
                        return;
                    }
                }

                resolve({
                    status: res.statusCode ?? 0,
                    body: response,
                    headers: res.headers,
                });
            });

            res.on('error', onError);
        };

        if (isHttp) {
            const base = new URL(req.baseUrl!);
            const basePath = base.pathname.endsWith('/')
                ? base.pathname.slice(0, -1)
                : base.pathname;
            const target = `${base.origin}${basePath}${req.path}`;

            const request = (lib as typeof https).request(
                target,
                {
                    method: req.method,
                    headers: req.headers,
                },
                onResponse,
            );

            request.on('error', onError);
            attachTimeout(request);
            writeBody(request);
            request.end();
            return;
        }

        const request = (lib as typeof http).request(
            {
                method: req.method,
                headers: req.headers,
                socketPath: req.socketPath,
                path: req.path,
            },
            onResponse,
        );

        request.on('error', onError);
        attachTimeout(request);
        writeBody(request);
        request.end();
    });
