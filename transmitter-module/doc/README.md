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
RUST_LOG=debug ENTANGLE_RABBITMQ_USER=guest ENTANGLE_RABBITMQ_PASSWORD=guest target/release/solana_keeper_module listener --config transmitter-common-module/doc/listener-config.yml
```

### Run executor

```sh
ENTANGLE_RABBITMQ_PASSWORD=guest;ENTANGLE_RABBITMQ_USER=guest;ENTANGLE_SOLANA_PAYER=4pewL6uTRV6g7SUa5B9QJVLHwhpXvnwAwrzxJaTA2g4WUosYZVyueEhAe5naFFhB1mtVet5fj9v6sRy9BUEzSuRt;RUST_LOG=info,transmitter_module=debug
```

Where `ENTANGLE_SOLANA_PAYER` is executor keypair encoded in base58, it's a test account that is also available
at `tests/accounts`

### Test executor

```sh
export ENTANGLE_RABBITMQ_PASSWORD=guest
export ENTANGLE_RABBITMQ_USER=guest
export RUST_LOG="info,test_publisher=debug"
cargo run --release --package test-publisher -- init-owned-counter --config transmitter-test-publisher/publisher-config.yml
cargo run --release --package test-publisher -- increment-owned-counter --config transmitter-test-publisher/publisher-config.yml  --value 2 --times 1
```

### Update extensions

To update the internal state without stopping the executor service, it is possible to reload the extension list from the
configuration by sending a SIGHUP.

```she
pgrep -a transmitter
105971 target/debug/transmitter-module executor --config transmitter-module/doc/executor-config.yaml
kill -1 105971
```

## Docker environment

For users who prefer not to build tools from source, there is also an option to run them in a docker environment

### Build solana test validator image

This image is a functional solana node that can be used for testing.
Photon messaging and onefunc contracts with test accounts are installed and initialized on it

```sh
docker build -t entangle:solana -f docker/Dockerfile_solana .
```

### Build solana module image

```sh
docker build -t entangle:solana-module -f docker/Dockerfile_module .
```

### Start test environment with docker-compose

```sh
docker-compose -f docker/docker-compose.yml up 
```

### Produce an operation with test publisher

For the test purposes the publishing signed operation data is also provided

```sh
docker run --network entangle -e "ENTANGLE_RABBITMQ_USER=guest"\
                              -e "ENTANGLE_RABBITMQ_PASSWORD=guest"\
                              -e "RUST_LOG=debug" \
           --rm -it --entrypoint './test-publisher'  --name publisher entangle:solana-module\
           init-owned-counter --config publisher-config.yml
```

```sh
docker run --network entangle -e "ENTANGLE_RABBITMQ_USER=guest"\
                              -e "ENTANGLE_RABBITMQ_PASSWORD=guest"\
                              -e "RUST_LOG=debug" \
           --rm -it --entrypoint './test-publisher'  --name publisher entangle:solana-module\
           increment-owned-counter --value 2 --times 1 --config publisher-config.yml
```
