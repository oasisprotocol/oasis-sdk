import * as http from 'node:http';
import * as https from 'node:https';
import {readFileSync} from 'node:fs';
import {resolve as resolvePath} from 'node:path';

/** Default Unix domain socket path. */
export const ROFL_SOCKET_PATH = '/run/rofl-appd.sock';

/** Key kinds supported by /rofl/v1/keys/generate. */
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

export class RoflClient {
    private readonly url: string;
    private readonly timeoutMs: number;
    private readonly transport?: Transport;
    private readonly userAgent: string;

    constructor(opts: RoflClientOptions = {}) {
        this.url = opts.url ?? '';
        this.timeoutMs = opts.timeoutMs ?? 60_000;
        this.transport = opts.transport;
        this.userAgent = opts.userAgent ?? DEFAULT_USER_AGENT;
    }

    /** Generate/fetch a cryptographic key from ROFL. Returns hex string (no 0x prefix). */
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

    /** Get all user-set metadata key-value pairs. */
    async getMetadata(): Promise<Record<string, string>> {
        const res = await this.appdRequest('GET', '/rofl/v1/metadata');

        if (!res || typeof res !== 'object' || res === null) {
            throw new Error('Invalid response from ROFL metadata');
        }

        return res as Record<string, string>;
    }

    /** Replace metadata key-value pairs (triggers registration refresh if changed). */
    async setMetadata(metadata: Record<string, string>): Promise<void> {
        await this.appdRequest('POST', '/rofl/v1/metadata', metadata);
    }

    /** Convenience: Returns ROFL app ID (bech32). */
    async getAppId(): Promise<string> {
        const res = await this.appdRequest('GET', '/rofl/v1/app/id');

        if (typeof res !== 'string') {
            throw new Error('Invalid response while fetching ROFL app id');
        }

        return res;
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

        const transport = this.transport ?? nodeTransport;
        const {status, body: responseBody} = await transport(transportRequest);

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
