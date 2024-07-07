import json, sys

photon = "./target/idl/photon.json"
onefunc = "./target/idl/onefunc.json"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = address
    open(path, "w").write(json.dumps(metadata))

fix(photon, "pccm961CjaR7T7Hcht9omrXQb9w54ntJo95FFT7N9AJ")
fix(onefunc, "EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ")
