#!/bin/sh
anchor build
python3 fixmetadata.py
anchor deploy -p photon --provider.cluster devnet
anchor migrate --provider.cluster devnet
