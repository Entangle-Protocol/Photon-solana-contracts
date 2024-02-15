import json
photon_address = "3cAFEXstVzff2dXH8PFMgm81h8sQgpdskFGZqqoDgQkJ"
onefunc_address = "EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = address
    open(path, "w").write(json.dumps(metadata))

fix("./target/idl/photon.json", photon_address)
fix("./target/idl/onefunc.json", onefunc_address)