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

const statusMD = new grpc.web.MethodDescriptor(
    '/oasis-core.NodeController/GetStatus',
    grpc.web.MethodType.UNARY,
    Object,
    Object,
    serializeRequestCBOR,
    deserializeResponseCBOR
);

const base = 'http://localhost:42280';
const oc = new grpc.web.GrpcWebClientBase();

function status(request) {
    return oc.unaryCall(base + statusMD.name, request, null, statusMD);
}

(async function () {
    try {
        const response = await status({});
        console.log(response);
    } catch (e) {
        console.error(e);
    }
})();
