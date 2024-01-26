import json
address = "CQ9k5uzuZycAHiAiLrabRdNYi4JuHWCHieTxJxFk1vH8"
metadata = json.loads(open("./target/idl/photon.json", "r").read())
metadata["metadata"] = {}
metadata["metadata"]["address"] = address
open("./target/idl/photon.json", "w").write(json.dumps(metadata))
