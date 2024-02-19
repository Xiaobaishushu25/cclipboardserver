use crate::message::message::Message;
use crate::message::message::Message::{ClipboardMessage, DeviceChangeResponseMessage};
use bytes::Buf;
use log::info;
use std::collections::HashSet;
use std::io::Cursor;
use std::io::SeekFrom::Start;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time;
use tokio::time::Instant;

#[test]
fn test_cursor() {
    let bytes = "nihao 1245748".as_bytes();
    info!("bytes {:?} len {}", bytes, bytes.len());
    let mut buf = Cursor::new(bytes);
    let first = buf.get_u8();
    info!("first {:?}", first);
    let len = buf.get_ref().len();
    info!("len {:?}", len);
}
#[test]
fn test_set() {
    let mut banned_ips = HashSet::new();
    banned_ips.insert("172.104.39.21");
    let string = String::from("172.104.39.21");
    if banned_ips.contains(string.as_str()) {
        info!("---")
    };
}
#[test]
fn test_match() {
    let message = ClipboardMessage("ui".into());
    // if (message ==Message::ClipboardMessage){
    //
    // }
    match message {
        ClipboardMessage(_) | DeviceChangeResponseMessage(_, _) => send(),
        _ => {}
    };
}
fn send() {
    println!("asd")
}
#[test]
fn test_adr() {
    println!("{}", SocketAddr::from_str("127.0.0.1:56085").unwrap());
}
#[tokio::test]
async fn test_cancel() {
    println!("开始了");
    let start = Instant::now() + Duration::from_millis(15);
    let mut heartbeat_interval = time::interval_at(start, Duration::from_secs(15));
    heartbeat_interval.tick().await;
    println!("结束了");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("无法读取输入");
}
