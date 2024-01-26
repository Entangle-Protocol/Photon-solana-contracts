#!/bin/sh
anchor build
python3 fixmetadata.py
anchor test --skip-build
