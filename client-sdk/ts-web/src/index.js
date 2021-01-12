const cbor = require('cbor-js');
const grpc = {};
grpc.web = require('grpc-web');

function serializeRequestCBOR(req) {
    return new Uint8Array(cbor.encode(req));
}

function deserializeResponseCBOR(u8) {
    let buf;
    if (u8.byteOffset === 0 && u8.byteLength === u8.buffer.byteLength) {
        buf = u8.buffer;
    } else {
        buf = u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
    }
    return cbor.decode(buf);
}

const md = new grpc.web.MethodDescriptor(
    '/oasis-core.Staking/Delegations',
    grpc.web.MethodType.UNARY,
    Object,
    Object,
    serializeRequestCBOR,
    deserializeResponseCBOR
);

const base = 'http://localhost:42280';
const oc = new grpc.web.GrpcWebClientBase();

function invoke(request) {
    return oc.unaryCall(base + md.name, request, null, md);
}

(async function () {
    try {
        const response = await invoke({
            owner: new Uint8Array([0,127,77,70,174,39,53,254,142,111,175,175,146,245,62,236,64,75,136,212,47]),
            height: 1920228,
        });
        console.log(response);
    } catch (e) {
        console.error(e);
    }
})();
