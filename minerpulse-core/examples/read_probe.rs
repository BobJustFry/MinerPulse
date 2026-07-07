//! Quick CLI: `cargo run -p minerpulse-core --example read_probe -- 192.168.35.42`
use minerpulse_core::{fetch_with_detect, FetchOptions, TcpCgminerClient};

fn main() {
    let ip = std::env::args().nth(1).unwrap_or_else(|| "192.168.35.42".into());
    let port: u16 = 4028;

    let client = TcpCgminerClient::default();

    let detect = FetchOptions {
        luci_auth: None,
        fast_poll: true,
        fetch_chips: false,
    };
    let t = std::time::Instant::now();
    match fetch_with_detect(&client, &ip, port, &detect) {
        Ok(s) => {
            println!(
                "OK ip={ip} vendor={:?} model={} ghs={} boards={} pools={} chips={} {:?}",
                s.identity.vendor,
                s.identity.model,
                s.hashrate.current_ghs,
                s.boards.len(),
                s.pools.len(),
                s.board_chips.len(),
                t.elapsed()
            );
            let json = serde_json::to_string(&s).expect("serialize");
            println!("json_bytes={}", json.len());
        }
        Err(e) => println!("ERR ip={ip} {e:?} {:?}", t.elapsed()),
    }
}
