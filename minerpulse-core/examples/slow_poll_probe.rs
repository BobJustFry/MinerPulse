use minerpulse_core::{fetch_with_detect, FetchOptions, TcpCgminerClient};
fn main() {
    let client = TcpCgminerClient::default();
    let opts = FetchOptions { luci_auth: None, fast_poll: false, fetch_chips: false, cancel: None };
    let t = std::time::Instant::now();
    let s = fetch_with_detect(&client, "192.168.35.42", 4028, &opts).unwrap();
    println!("slow_poll ghs={} {:?} vendor={:?}", s.hashrate.current_ghs, t.elapsed(), s.identity.vendor);
}
