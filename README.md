# Soros

## Overview

This project is a distributed key-value store system that facilitates storage and retrieval of key-value pairs across a network of nodes using peer-to-peer communication and an authorative server. 

## Components

- Node: Represents an individual server or peer in the distributed network. Each node is responsible for storing a subset of the data and communicating with other nodes.
- Distributed Storage: Data is distributed across multiple nodes in the network.
- Peer-to-Peer Communication: Nodes communicates with each other directly to share data.
- Registry Server: Serve as data registry, providing information about data location, ensuring the distribution of read and write requests using a fair read-write lock mechanism. 

## Getting started

Run the registry server:

```bash
cargo run --bin server 8080
```

Run the client examples:

```bash
cargo run --bin client localhost 8080
```

Run the tests:

```bash
cargo run test
```

Run the registry benchmarks with the plots:

```bash
cargo run --bin registry_benchmark && python3 scripts/registry_plot.py
```
