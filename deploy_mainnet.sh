#!/bin/bash
solana program deploy \
    --with-compute-unit-price 10000000 --commitment confirmed \
    --buffer recover.json \
    --program-id ./target/deploy/photon-keypair.json \
    ./target/deploy/photon.so