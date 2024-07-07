#!/bin/sh
python3 fixmetadata.py
solana-test-validator -q --reset --bpf-program  keys/onefunc-keypair.json /deploy/onefunc.so \
                                 --bpf-program keys/photon-keypair.json /deploy/photon.so &
sleep 1
anchor test --skip-local-validator --skip-build --skip-deploy
