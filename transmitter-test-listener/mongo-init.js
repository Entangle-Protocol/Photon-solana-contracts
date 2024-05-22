db.createCollection('last_processed_blocks', {capped: false});
db.last_processed_blocks.createIndex({"direction": 1, "chain": 1}, {unique: true});
db.last_processed_blocks.insert({
    "direction": "from",
    "chain": "100000000000000000000",
    "last_watched_block": null,
    "last_processed_block": null
})
db.last_processed_blocks.insert({"direction": "to", "chain": "100000000000000000000", "last_processed_block": null})
