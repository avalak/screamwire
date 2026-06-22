use ringbuf::{HeapRb, traits::Split};
use std::thread;

mod pw;
mod scream;

const MULTICAST_ADDR: &str = "239.255.77.77:4010";
const SENDER_BIND_ADDR: &str = "0.0.0.0:0";

// Ring buffer size (number of packets, ~60 ms buffering)
const RING_BUFFER_PACKETS: usize = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ScreamWire sender starting...");
    println!("Multicast target: {}", MULTICAST_ADDR);

    // Ring buffer
    let rb = HeapRb::<u8>::new(scream::PACKET_SIZE * RING_BUFFER_PACKETS);
    let (producer, consumer) = rb.split();

    // Start sender thread
    let target_addr: std::net::SocketAddr = MULTICAST_ADDR.parse()?;
    let bind_addr: std::net::SocketAddr = SENDER_BIND_ADDR.parse()?;
    let _sender_thread = thread::spawn(move || scream::send_loop(consumer, target_addr, bind_addr));

    // Run PipeWire virtual sink (blocks until exit)
    pw::run_virtual_sink(producer, scream::RATE, scream::CHANNELS)?;

    Ok(())
}
