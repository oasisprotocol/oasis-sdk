pragma solidity ^0.8.0;

contract Test {
    address constant private GENERATE = 0x0100000000000000000000000000000000000005;
    address constant private SIGN = 0x0100000000000000000000000000000000000006;
    address constant private VERIFY = 0x0100000000000000000000000000000000000007;
    constructor() {}
    function generate(uint method, bytes memory seed) private view returns (bytes memory publicKey, bytes memory privateKey) {
        (bool ok, bytes memory output) = GENERATE.staticcall(abi.encode(method, seed));
        if (!ok) {
            revert("generate failed");
        }
        return abi.decode(output, (bytes, bytes));
    }

    function sign(uint method, bytes memory privateKey, bytes memory context, bytes memory message) private view returns (bytes memory signature) {
        (bool ok, bytes memory output) = SIGN.staticcall(abi.encode(method, privateKey, context, message));
        if (!ok) {
            revert("sign failed");
        }
        return output;
    }

    function verify(uint method, bytes memory publicKey, bytes memory context, bytes memory message, bytes memory signature) private view returns (bool result) {
        (bool ok, bytes memory output) = VERIFY.staticcall(abi.encode(method, publicKey, context, message, signature));
        if (!ok) {
            revert("verify failed");
        }
        return abi.decode(output, (bool));
    }

    function test() public view returns (string memory) {
        bytes memory seed = hex"6d792073656372657420736565642076616c75652c202069742773206d696e65";
        bytes memory context = hex"612074757262756c656e7420636f6e7465787420746f206c6976652077697468";
        bytes memory message = hex"746865206d6573736167652c20746f206265207369676e6564206279206b6579";

        for (uint method = 0; method < 5; method++) {
            bytes memory publicKey;
            bytes memory privateKey;
            (publicKey, privateKey) = generate(method, seed);
            bool result;
            if (method < 2 || method == 3) {
                bytes memory signature = sign(method, privateKey, context, message);
                result = verify(method, publicKey, context, message, signature);
            } else {
                bytes memory short = hex"6d792073656372657420736565642076616c75652c202069742773206d696e65";
                bytes memory long = hex"6d792073656372657420736565642076616c75652c202069742773206d696e656d792073656372657420736565642076616c75652c202069742773206d696e65";
                bytes memory hash = (method == 2) ? long : short;
                bytes memory signature = sign(method, privateKey, hash, hex"");
                result = verify(method, publicKey, hash, hex"", signature);
            }
            if (!result) {
                revert(string(abi.encode(method)));
            }
        }
        return "ok";
    }
}