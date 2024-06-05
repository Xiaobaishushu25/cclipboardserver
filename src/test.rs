use crate::message::message::{DeviceInfo, Message};
use crate::message::message::Message::{ClipboardMessage, DeviceChangeResponseMessage};
use bytes::{Buf, BytesMut};
use log::info;
use std::collections::{HashMap, HashSet};
use std::sync::{ Mutex, RwLock};

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time;
use tokio::time::{Instant, sleep};
use crate::handler::channel_manage::ChannelManage;

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
#[test]
fn test_change_map() {
    let mut map = HashMap::new();
    map.insert("test".to_string(),1);
    if let Some(num) = map.get_mut("test"){
        *num = *num +1;
    }
    println!("{:?}",map)
}
#[tokio::test]
async fn test_tcp() {
    let addr = "127.0.0.1:8888";
    match TcpStream::connect(addr).await {
        Ok(mut stream) => {
            stream.write_all("wo cesini".as_bytes()).await.unwrap();
            // 尝试从服务器读取数据
            let mut buffer = [0; 1024];
            match stream.read(&mut buffer).await {
                Ok(0) => println!("连接已关闭"),
                Ok(n) => println!("收到 {} 字节数据: {:?}", n, &buffer[..n]),
                Err(e) => println!("读取错误: {}", e),
            }
        }
        Err(e) => {
            println!("{}", e)
        }
    };
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("无法读取输入");
}
#[tokio::test]
async fn test_context() {
    println!("开始");
    let mut heartbeat_interval = time::interval(Duration::from_secs(25));
    tokio::spawn(async move {
        loop {
            heartbeat_interval.tick().await;
            println!("tick")
        } 
    });
    println!("结束");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("无法读取输入");
}
#[tokio::test]
async fn test_context2() {
    use std::io::{self, Write}; // 引入 `Write` trait

    println!("开始");
    io::stdout().flush().unwrap(); // 刷新标准输出缓冲区

    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(25));
    tokio::spawn(async move {
        loop {
            heartbeat_interval.tick().await;
            println!("tick");
            io::stdout().flush().unwrap(); // 刷新标准输出缓冲区
        }
    });

    println!("结束");
    io::stdout().flush().unwrap(); // 刷新标准输出缓冲区

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("无法读取输入");
}
