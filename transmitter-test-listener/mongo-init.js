db.createCollection('last_processed_blocks', { capped: false });
db.last_processed_blocks.createIndex({ "direction": 1, "chain": 1, "key": 1 }, { unique: true });
db.last_processed_blocks.insert({ "direction": "from", "chain": "0x56bc75e2d63100000", "key": "last_processed_block", "value": null })
db.last_processed_blocks.insert({ "direction": "to", "chain": "0x56bc75e2d63100000", "key": "last_processed_block", "value": null })
