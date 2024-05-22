db.createCollection('last_processed_blocks', {capped: false});
db.last_processed_blocks.createIndex({"direction": 1, "chain": 1}, {unique: true});
db.last_processed_blocks.insert({
    "direction": "from",
    "chain": "11000000000000000501",
    "last_watched_block": null,
    "last_processed_block": null
})
db.last_processed_blocks.insert({"direction": "to", "chain": "11000000000000000501", "last_processed_block": null})
