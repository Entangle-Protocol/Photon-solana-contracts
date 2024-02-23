#!/bin/sh
anchor build
python3 fixmetadata.py
solana-test-validator -q --reset --bpf-program target/deploy/onefunc-keypair.json target/deploy/onefunc.so --bpf-program target/deploy/photon-keypair.json target/deploy/photon.so &
sleep 2
anchor test --skip-local-validator --skip-build --skip-deploy
pkill solana-test
