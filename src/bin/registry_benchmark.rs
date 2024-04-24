use std::{
    fs::{self, File},
    thread,
    time::{Duration, Instant},
};

use soros::{
    handle_wait_error,
    protocol_client::{ProtocolClient, ProtocolClientError},
    registry_server::{RegistryServer, RegistryServerError},
};

fn server() -> Result<(), RegistryServerError> {
    let mut registry_server = RegistryServer::new(8888)?;
    registry_server.bind()?;
    Ok(())
}

fn create_client() -> Result<ProtocolClient, ProtocolClientError> {
    ProtocolClient::new("localhost", 8888)
}

fn read_client(
    nb_requests: u32,
) -> Result<(Duration, u32), ProtocolClientError> {
    let mut protocol_client = create_client()?;
    let mut mean_access_time = Duration::default();
    let mut nb_blocks = 0;

    for _ in 0..nb_requests {
        let start_time = Instant::now();

        protocol_client.registry_read(1)?;
        handle_wait_error!(protocol_client.registry_await_read(), {
            nb_blocks += 1;
            protocol_client.registry_await_read()?;
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

        protocol_client.registry_write(1)?;
        handle_wait_error!(protocol_client.registry_await_write(), {
            nb_blocks += 1;
            protocol_client.registry_await_write()?;
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

fn bench_readers_writers(
    nb_readers: u32,
    nb_writers: u32,
    nb_requests: u32,
    wtr: &mut csv::Writer<File>,
) {
    let mut read_handles = vec![];
    let mut readers_access_time = Duration::default();
    let mut nb_blocks_readers = 0;
    let mut write_handles = vec![];
    let mut writers_access_time = Duration::default();
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
        readers_access_time += mean_access_time;
        nb_blocks_readers += nb_blocks;
    }
    for handle in write_handles {
        let (mean_access_time, nb_blocks) =
            handle.join().unwrap().expect("write failed");
        writers_access_time += mean_access_time;
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

    let mut mean_readers_access_time = Duration::default();
    if nb_readers != 0 {
        mean_readers_access_time = readers_access_time / nb_readers;
    }
    let mut mean_writers_access_time = Duration::default();
    if nb_writers != 0 {
        mean_writers_access_time = writers_access_time / nb_writers;
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
        colorize!(
            YELLOW,
            format!(
                "{:?} (total: {:?})",
                mean_readers_access_time, readers_access_time
            )
        ),
        colorize!(BLUE, ratio_blocked_readers),
        nb_blocks_readers,
        nb_access_readers
    );
    println!(
        "- Mean write access time: {}, access blocked: {} ({}/{})",
        colorize!(
            YELLOW,
            format!(
                "{:?} (total: {:?})",
                mean_writers_access_time, writers_access_time
            )
        ),
        colorize!(BLUE, ratio_blocked_writers),
        nb_blocks_writers,
        nb_access_writers
    );

    // write data to csv for plots
    wtr.write_record(&[
        format!("{}/{}", nb_readers, nb_writers),
        "readers".to_string(),
        mean_readers_access_time.as_micros().to_string(),
        ratio_blocked_readers.to_string(),
    ])
    .unwrap();
    wtr.write_record(&[
        format!("{}/{}", nb_readers, nb_writers),
        "writers".to_string(),
        mean_writers_access_time.as_micros().to_string(),
        ratio_blocked_writers.to_string(),
    ])
    .unwrap();
}

fn main() -> Result<(), ProtocolClientError> {
    env_logger::init();

    let server_thread = thread::spawn(server);

    // init
    let mut protocol_client = create_client()?;
    protocol_client.registry_create(1)?;

    fs::create_dir_all("generated").unwrap();
    let file = File::create("generated/registry-bench.csv").unwrap();
    let mut wtr = csv::Writer::from_writer(file);
    wtr.write_record(["ratio", "access_type", "access_time", "block_ratios"])
        .unwrap();

    let nb_requests = 1000;
    bench_readers_writers(100, 0, nb_requests, &mut wtr);
    bench_readers_writers(0, 100, nb_requests, &mut wtr);
    bench_readers_writers(80, 20, nb_requests, &mut wtr);
    bench_readers_writers(20, 80, nb_requests, &mut wtr);
    bench_readers_writers(50, 50, nb_requests, &mut wtr);

    // cleanup
    protocol_client.registry_delete(1)?;
    protocol_client.registry_stop()?;

    server_thread.join().unwrap().expect("server failed");

    Ok(())
}
