# Soros

## Overview

This project is a distributed key-value store system that facilitates storage and retrieval of key-value pairs across a network of nodes using peer-to-peer communication and an authorative server. 

## Components

- Distributed Storage: Data is distributed across multiple nodes in the network.
- Peer-to-Peer Communication: Nodes communicates with each other directly to share data.
- Registry Server: Serve as data registry, ensure the distribution of read and write requests using a fair read-write lock mechanism. 

### Components

- Node: Represents an individual server or peer in the distributed network. Each node is responsible for storing a subset of the data, processing requests, and communicating with other nodes.
- Communication Layer: Facilitates communication between nodes using a peer-to-peer protocol. This layer handles message routing, synchronization, and error handling.
- Authority Server: Provides information about the location of data, assisting clients in locating the appropriate node for key-value operations.

## Getting started

Run the registry server:

```bash
cargo run --bin server
```

Run the client examples:

```bash
cargo run --bin client
```

Run the tests:

```bash
cargo run test
```

Run the registry benchmarks with the plots:

```bash
cargo run --bin registry_benchmark && python3 scripts/registry_plot.py
```
