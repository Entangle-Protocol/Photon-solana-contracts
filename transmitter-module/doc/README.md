# Test

The following instructions describe how to create a test environment to verify that the photon messaging events
are captured and passed on to the keeper base module

## Build

### Build photon messaging aggregation spotter and onefunc programs

```sh
anchor build
```


### Build solana-keeper-module

```
cargo build --bin transmitter_module
```

## Run and test

### Solana test validator

To start a solana test validator with compiled previously solana programs the following command can be used

```sh 
python fixmetadata.py
solana-test-validator --reset --bpf-program target/deploy/onefunc-keypair.json target/deploy/onefunc.so --bpf-program target/deploy/photon-keypair.json target/deploy/photon.so
```

### Initialize photon messaging state

The Photon messaging on-chain state is initialized by calling the photon tests as follows

```sh
anchor test --skip-local-validator --skip-build --skip-deploy
```

### Rabbitmq

```sh 
 docker run -it --rm --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3.12-management
```

or if rabbitmq container has been initialized before

```sh
docker start rabbitmq 
```

### Solana keeper module

```sh
RUST_LOG=debug ENTANGLE_RABBITMQ_USER=guest ENTANGLE_RABBITMQ_PASSWORD=guest target/release/solana_keeper_module listener --config transmitter-common-module/doc/listener-config.yaml
```

### Run executor

```sh
ENTANGLE_RABBITMQ_PASSWORD=guest;ENTANGLE_RABBITMQ_USER=guest;ENTANGLE_SOLANA_PAYER=4pewL6uTRV6g7SUa5B9QJVLHwhpXvnwAwrzxJaTA2g4WUosYZVyueEhAe5naFFhB1mtVet5fj9v6sRy9BUEzSuRt;RUST_LOG=info,transmitter_module=debug
```

Where `ENTANGLE_SOLANA_PAYER` is executor keypair encoded in base58, it's a test account that is also available at `tests/accounts`

### Test executor

```sh
export NTANGLE_RABBITMQ_PASSWORD=guest
export ENTANGLE_RABBITMQ_USER=guest
export RUST_LOG="info,test_publisher=debug"
cargo run --release --package test-publisher -- init-owned-counter --config transmitter-module/doc/listener-config.yaml
cargo run --release --package test-publisher -- increment-owned-counter --config transmitter-module/doc/listener-config.yaml --value 2
```
