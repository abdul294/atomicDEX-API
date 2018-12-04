use common::bits256;
use common::for_tests::wait_for_log;
use common::mm_ctx::MmCtx;
use gstuff::now_float;
use libc::{self, c_void};
use rand::{self, Rng};
use std::mem::zeroed;
use std::net::{Ipv4Addr, SocketAddr};
use std::ptr::null_mut;
use std::slice::from_raw_parts;
use std::thread;
use std::time::Duration;

pub fn test_dht() {
    // Create the Alice and Bob contexts.

    let alice = MmCtx::new (json! ({}), SocketAddr::new (Ipv4Addr::new (127, 0, 0, 1) .into(), 123));

    let bob = MmCtx::new (json! ({}), SocketAddr::new (Ipv4Addr::new (127, 0, 0, 1) .into(), 123));

    let mut rng = rand::thread_rng();

    // Initialize the DHT on both, using different preferred ports.

    let mut alice_key: bits256 = unsafe {zeroed()};
    unsafe {rng.fill (&mut alice_key.bytes[..])}
    unwrap! (::initialize (&alice, 9999, alice_key, 2111, 0));

    let mut bob_key: bits256 = unsafe {zeroed()};
    unsafe {rng.fill (&mut bob_key.bytes[..])}
    unwrap! (::initialize (&bob, 9999, bob_key, 2112, 0));

    unwrap! (wait_for_log (&alice.log, 33., &|en| en.contains ("[dht-boot] DHT bootstrap ... Done.")));
    unwrap! (wait_for_log (&bob.log, 33., &|en| en.contains ("[dht-boot] DHT bootstrap ... Done.")));

    let max_length = 992 /* (1000 - bencode overhead - checksum) */ * 253 /* Compatible with (1u8..) */ - 1 /* space for number_of_chunks */;
    let tested_lengths = [1, 987, 32 * 1024, 96 * 1024, max_length];
    for message_len in tested_lengths.iter() {
        // Send a message to Bob.

        let message: Vec<u8> = (0..*message_len).map (|_| rng.gen()) .collect();

        unwrap! (::bind (&alice, 1, bob_key));
        let alice_ctx = unwrap! (alice.ffi_handle());
        ::peers_clock_tick_compat (alice_ctx, 1);
        ::peers_send_compat (alice_ctx, 1, message.as_ptr(), message.len() as i32);

        // Get that message from Alice.

        unwrap! (::bind (&bob, 1, alice_key));
        let bob_ctx = unwrap! (bob.ffi_handle());
        ::peers_clock_tick_compat (bob_ctx, 1);
        let started_at = now_float();
        loop {
            let mut data: *mut u8 = null_mut();
            let rc = ::peers_recv_compat (bob_ctx, 1, &mut data);
            if rc < 0 {panic! ("peers_recv_compat error: {}", rc)}
            if rc > 0 {
                let payload: Vec<u8> = unsafe {from_raw_parts (data, rc as usize)} .into();
                unsafe {libc::free (data as *mut c_void)}
                if payload == &message[..] {break}
            }
            if now_float() - started_at > (33 + unwrap! (tested_lengths.iter().max()) / 992 * 2) as f64 {
                panic! ("Out of time waiting for DHT payload")
            }
            thread::sleep (Duration::from_millis (200))
        }
    }
}