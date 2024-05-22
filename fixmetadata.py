import json, sys

photon = "./target/idl/photon.json"
onefunc = "./target/idl/onefunc.json"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = address
    open(path, "w").write(json.dumps(metadata))

if len(sys.argv) == 2 and sys.argv[1] == "mainnet":
    fix(photon, "pccm961CjaR7T7Hcht9omrXQb9w54ntJo95FFT7N9AJ")
else:
    fix(photon, "JDxWYX5NrL51oPcYunS7ssmikkqMLcuHn9v4HRnedKHT")
    fix(onefunc, "EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ")
