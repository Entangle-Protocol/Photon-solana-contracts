import json
photon_address = "9pGziQeWKwruehVXiF9ZHToiVs9iv7ajXeFFaPiaLkpD"
onefunc_address = "6cBwMuV2hTAVAXYSqYXULuXitknhzJYu3QXjuH9mKaLg"

def fix(path, address):
    metadata = json.loads(open(path, "r").read())
    metadata["metadata"] = {}
    metadata["metadata"]["address"] = photon_address
    open(path, "w").write(json.dumps(metadata))

fix("./target/idl/photon.json", photon_address)
fix("./target/idl/onefunc.json", onefunc_address)