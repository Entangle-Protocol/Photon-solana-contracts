import json
photon_address = "JDxWYX5NrL51oPcYunS7ssmikkqMLcuHn9v4HRnedKHT"
onefunc_address = "EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = address
    open(path, "w").write(json.dumps(metadata))

fix("./target/idl/photon.json", photon_address)
fix("./target/idl/onefunc.json", onefunc_address)