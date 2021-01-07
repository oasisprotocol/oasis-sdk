const grpc = {};
grpc.web = require('grpc-web');

const statusMD = new grpc.web.MethodDescriptor(
    '/oasis-core.NodeController/GetStatus',
    grpc.web.MethodType.UNARY,
    Object,
    Object,
    (v) => new Uint8Array([0xf6]),
    (v) => v
);

const base = 'http://localhost:42280';
const oc = new grpc.web.GrpcWebClientBase();

function status(request) {
    return oc.unaryCall(base + statusMD.name, request, {}, statusMD);
}

(async function () {
    try {
        const response = await status({});
        console.log(response);
    } catch (e) {
        console.error(e);
    }
})();
