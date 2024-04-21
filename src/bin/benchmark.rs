use std::{
    thread,
    time::{Duration, Instant},
};

use soros::{
    handle_wait,
    protocol_client::{ProtocolClient, ProtocolClientError},
    registry_server::{RegistryServer, RegistryServerError},
};

fn server() -> Result<(), RegistryServerError> {
    let mut registry_server = RegistryServer::new("localhost", 8888)?;
    registry_server.bind()?;
    Ok(())
}

fn create_client() -> Result<ProtocolClient, ProtocolClientError> {
    ProtocolClient::new("localhost", 8888)
}

fn init() -> Result<(), ProtocolClientError> {
    let mut protocol_client = create_client()?;
    protocol_client.registry_create(1)?;
    Ok(())
}

fn cleanup() -> Result<(), ProtocolClientError> {
    let mut protocol_client = create_client()?;
    protocol_client.registry_delete(1)?;
    protocol_client.registry_stop()?;
    Ok(())
}

fn read_client(
    nb_requests: u32,
) -> Result<(Duration, u32), ProtocolClientError> {
    let mut protocol_client = create_client()?;
    let mut mean_access_time = Duration::default();
    let mut nb_blocks = 0;

    for _ in 0..nb_requests {
        let start_time = Instant::now();
        handle_wait!(protocol_client.registry_read(1), {
            nb_blocks += 1;
            protocol_client.registry_await_read(1)?;
        });
        protocol_client.registry_release(1)?;
        mean_access_time += start_time.elapsed();
    }

    mean_access_time /= nb_requests;
    Ok((mean_access_time, nb_blocks))
}

fn write_client(
    nb_requests: u32,
) -> Result<(Duration, u32), ProtocolClientError> {
    let mut protocol_client = create_client()?;
    let mut mean_access_time = Duration::default();
    let mut nb_blocks = 0;

    for _ in 0..nb_requests {
        let start_time = Instant::now();
        handle_wait!(protocol_client.registry_write(1), {
            nb_blocks += 1;
            protocol_client.registry_await_write(1)?;
        });
        protocol_client.registry_release(1)?;
        mean_access_time += start_time.elapsed();
    }

    mean_access_time /= nb_requests;
    Ok((mean_access_time, nb_blocks))
}

const BLUE: &str = "\x1b[34m";
const YELLOW: &str = "\x1b[33m";

macro_rules! colorize {
    ($color: expr, $number: expr) => {
        format!("{}{}{}", $color, $number, "\x1b[0m")
    };
}

fn bench_readers_writers(nb_readers: u32, nb_writers: u32, nb_requests: u32) {
    let mut read_handles = vec![];
    let mut mean_reader_access_time = Duration::default();
    let mut nb_blocks_readers = 0;
    let mut write_handles = vec![];
    let mut mean_writer_access_time = Duration::default();
    let mut nb_blocks_writers = 0;

    for _ in 0..nb_readers {
        read_handles.push(thread::spawn(move || read_client(nb_requests)));
    }
    for _ in 0..nb_writers {
        write_handles.push(thread::spawn(move || write_client(nb_requests)));
    }

    for handle in read_handles {
        let (mean_access_time, nb_blocks) =
            handle.join().unwrap().expect("read failed");
        mean_reader_access_time += mean_access_time;
        nb_blocks_readers += nb_blocks;
    }
    for handle in write_handles {
        let (mean_access_time, nb_blocks) =
            handle.join().unwrap().expect("write failed");
        mean_writer_access_time += mean_access_time;
        nb_blocks_writers += nb_blocks;
    }

    let total_workers = nb_readers + nb_writers;
    let ratio_readers = (nb_readers as f32) / (total_workers as f32) * 100.0;
    let ratio_writers = (nb_writers as f32) / (total_workers as f32) * 100.0;
    let nb_access_readers = nb_requests * nb_readers;
    let nb_access_writers = nb_requests * nb_writers;
    let mut ratio_blocked_readers =
        (nb_blocks_readers as f32) / (nb_access_readers as f32);
    if ratio_blocked_readers.is_nan() {
        ratio_blocked_readers = 0.0;
    }
    let mut ratio_blocked_writers =
        (nb_blocks_writers as f32) / (nb_access_writers as f32);
    if ratio_blocked_writers.is_nan() {
        ratio_blocked_writers = 0.0;
    }

    println!(
        "For a ratio of {} ({}/{}) readers and {} writers ({}/{}), with {} requests",
        colorize!(BLUE, format!("{}%", ratio_readers)),
        nb_readers,
        total_workers,
        colorize!(BLUE, format!("{}%", ratio_writers)),
        nb_writers,
        total_workers,
        colorize!(BLUE, nb_requests),
    );
    println!(
        "- Mean read access time: {}, access blocked: {} ({}/{})",
        colorize!(YELLOW, format!("{:?}", mean_reader_access_time)),
        colorize!(BLUE, ratio_blocked_readers),
        nb_blocks_readers,
        nb_access_readers
    );
    println!(
        "- Mean write access time: {}, access blocked: {} ({}/{})",
        colorize!(YELLOW, format!("{:?}", mean_writer_access_time)),
        colorize!(BLUE, ratio_blocked_writers),
        nb_blocks_writers,
        nb_access_writers
    );
}

fn main() {
    env_logger::init();

    let server_thread = thread::spawn(server);

    thread::spawn(init).join().unwrap().expect("init failed");

    let nb_requests = 100;
    bench_readers_writers(10, 0, nb_requests);
    bench_readers_writers(0, 10, nb_requests);
    bench_readers_writers(8, 2, nb_requests);
    bench_readers_writers(2, 8, nb_requests);
    bench_readers_writers(5, 5, nb_requests);

    thread::spawn(cleanup)
        .join()
        .unwrap()
        .expect("cleanup failed");

    server_thread.join().unwrap().expect("server failed");
}
