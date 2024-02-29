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
cargo build --bin solana_keeper_module
```

## Run

### Solana test validator

To start a solana test validator with compiled previously solana programs the following command can be used

```sh 
solana-test-validator --reset --bpf-program target/deploy/onefunc-keypair.json target/deploy/onefunc.so --bpf-program target/deploy/photon-keypair.json target/deploy/photon.so
```

### Rabbitmq

```sh 
 docker run -it --rm --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3.12-management
```

### Solana keeper module

```sh
RUST_LOG=debug ENTANGLE_RABBITMQ_USER=guest ENTANGLE_RABBITMQ_PASSWORD=guest target/release/solana_keeper_module listen --config solana-keeper-module/doc/config.yaml
```
