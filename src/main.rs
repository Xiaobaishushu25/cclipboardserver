mod app_errors;
mod clipboard;
mod handler;
mod message;
mod test;

use clipboard_master::ClipboardHandler;

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpSocket;

use crate::handler::handler::{ChannelMap, Context};

// fn main() {
//     let _ = Master::new(handler::new()).run();
// }
#[tokio::main]
async fn main() {
    // let listener = TcpListener::bind("127.0.0.1:8888").unwrap();
    let listener = {
        let addr = "127.0.0.1:8888";
        let backlog = 1024;

        let socket = TcpSocket::new_v4().unwrap();
        socket.bind(addr.parse().unwrap()).unwrap();
        socket.set_reuseaddr(true).unwrap();
        // socket.set_reuseport(true).unwrap();
        let listener = socket.listen(backlog).unwrap();
        listener
    };
    let channel_map = Arc::new(Mutex::new(ChannelMap::new()));
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        println!("与{:?}建立连接", stream);
        let arc_channel = channel_map.clone();
        tokio::spawn(async move {
            let mut context = Context::new(arc_channel, stream, addr);
            context.start_work().await;
            // tokio::spawn(async move{ context.start_work().await; })
            // context
        });
    }
    // let mut user_channel:HashMap<String,(Sender<(String, SocketAddr)>, Receiver<(String, SocketAddr)>)> = HashMap::with_capacity(50);
    // let mut user_channel:HashMap<String,Arc<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>> = HashMap::with_capacity(50);
    // let (tx, _rx) = broadcast::channel(50);
    // loop {
    //     let (mut stream, addr) = listener.accept().await.unwrap();
    //     println!("new client: {}", addr);
    //
    //     stream.set_nodelay(true).unwrap();
    //
    //     let tx = tx.clone();
    //     let mut rx = tx.subscribe();
    //
    //
    //     tokio::spawn(async move {
    //         let (reader, mut writer) = stream.split();
    //
    //         let mut reader = BufReader::new(reader);
    //
    //         let mut line = String::new();
    //
    //         loop {
    //             tokio::select! {
    //                 result = reader.read_line(&mut line) => {
    //                     if result.unwrap() == 0 {
    //                         break;
    //                     }
    //                     println!("msg received: {}", line);
    //                     if line.trim() == "quit" { break; }
    //                     tx.send((line.clone(), addr)).unwrap();
    //                     line.clear();
    //                 }
    //                 result = rx.recv() => match result{
    //                     Ok((line, addr_other)) => {
    //                         if addr != addr_other {
    //                             writer.write_all(line.as_bytes()).await.unwrap();
    //                         }
    //                     },
    //                     Err(RecvError::Lagged(_num)) => continue,
    //                     Err(_) => break,
    //                 }
    //             };
    //         }
    //         println!("client leave: {}", addr);
    //     });
    // }
}
// pub fn handle_connect(stream:TcpStream, addr:SocketAddr){
//     tokio::spawn(||{
//
//
//     });
// }
