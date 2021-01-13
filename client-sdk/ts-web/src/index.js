import * as cborg from 'cborg';
import * as grpcWeb from 'grpc-web';

const md = new grpcWeb.MethodDescriptor(
    '/oasis-core.Staking/Delegations',
    grpcWeb.MethodType.UNARY,
    Object,
    Object,
    cborg.encode,
    cborg.decode,
);

const base = 'http://localhost:42280';
const oc = new grpcWeb.GrpcWebClientBase();

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
