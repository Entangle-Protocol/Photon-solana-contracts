import json, sys

photon = "./target/idl/photon.json"
onefunc = "./target/idl/onefunc.json"
zerosum = "./target/idl/zerosum.json"
ngl_core = "./target/idl/ngl_core.json"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = address
    open(path, "w").write(json.dumps(metadata))

fix(photon, "Cc1AtsbqQrt9QiZRrMwzZTS4oMRXRWZrWBQsNNpmrj4R")
fix(onefunc, "QjB5Zuc3PasXPfdSta54GzKQa5yNiQk9TEmLUJEA2Xk")
fix(zerosum, "E81J4bjf8vHpioEGn75ysw895khGs5uaK3hDxPCe2z55")
fix(ngl_core, "FmHwfH7HvAoD7HNvwX71ffUF3G2ejJoPDcZr4kBu5Y2a")
