mod app_errors;
mod handler;
mod message;
mod test;
mod utils;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::handler::channel_manage::ChannelManage;
use log::{error, info};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpSocket;
use tokio::sync::broadcast;

use crate::handler::handler::Context;
use crate::handler::handler_test::ContextTest;
use crate::message::message::Message;

#[tokio::main]
async fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    // let listener = TcpListener::bind("127.0.0.1:8888").unwrap();
    let listener = {
        // let addr = "172.25.132.211:8888";
        let addr = "127.0.0.1:8888";
        info!("server bind addr：{}", addr);
        let backlog = 1024;
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind(addr.parse().unwrap()).unwrap();
        socket.set_reuseaddr(true).unwrap();
        // socket.set_reuseport(true).unwrap();
        socket.listen(backlog).unwrap()
    };
    // 创建一个HashSet来存储禁止的IP地址
    let mut banned_ips = HashSet::new();
    banned_ips.insert("172.104.39.21");
    // let channel_map = Arc::new(Mutex::new(PairChannelMap::new()));
    // 创建一个HashMap来存储一个IP的连接数量（偶尔会有未知ip疯狂用不同端口建立连接，我限定一个ip最多只能连十个端口，再多不处理）
    let mut ip_port_num = HashMap::with_capacity(10);
    let channel_manage = Arc::new(RwLock::new(ChannelManage::new()));
    loop {
        let (mut stream, addr) = listener.accept().await.unwrap();
        let ip = addr.ip().to_string();
        if banned_ips.contains(ip.as_str()) {
            info!("this is banned ip:{}",addr);
            let _ = stream.shutdown().await;
            continue;
        }
        if let Some(num) = ip_port_num.get_mut(&ip){
            if *num>10{
                error!("this ip {} connect too many port {}",addr,*num);
                let _ = stream.shutdown().await;
                continue;
            }else {
                *num = *num+1;
                info!("ip:{} port +1 {}",addr,*num+1);   
            }
        }else { 
            ip_port_num.insert(ip.clone(),1);
        }
        info!("connect with aadr:{}", addr);
        {
            //不知道这个锁什么时候释放，我给他划个范围
            let (tx, _) = broadcast::channel::<Message>(64);
            let mut write_guard = channel_manage.write().unwrap();
            write_guard.add_stream_channel(addr, tx.clone());
            info!("now connect device num is {}", write_guard.get_socket_num());
        }
        // let arc_channel_map = channel_map.clone();
        let arc_channel_manage = channel_manage.clone();
        tokio::spawn(async move {
            if let Some(num) = num_threads::num_threads() {
                info!("before:Current thread count: {}", num);
            } else {
                error!("Failed to get process info.");
            }
            let mut context = Context::new(arc_channel_manage, stream, addr);
            // let mut context = ContextTest::new(arc_channel_manage, stream, addr);
            if let Some(num) = num_threads::num_threads() {
                info!("after context :Current thread count: {}", num);
            } else {
                error!("Failed to get process info.");
            }
            context.send_ready().await;
            if let Some(num) = num_threads::num_threads() {
                info!("after send_ready():Current thread count: {}", num);
            } else {
                error!("Failed to get process info.");
            }
            context.start_work().await;
            if let Some(num) = num_threads::num_threads() {
                info!("after start_work:Current thread count: {}", num);
            } else {
                error!("Failed to get process info.");
            }
            // tokio::spawn(async move{ context.start_work().await; })
            // context
        });
    }
}
