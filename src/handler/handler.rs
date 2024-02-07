use crate::app_errors::AppError::IncompleteError;
use crate::app_errors::AppResult;
use crate::message::message::{DeviceInfo, Message};
use crate::message::message::Message::{ClipboardMessage, NoPairDeviceResponseMessage, PairCodeResponseMessage, PairCreateMessage, PairDeviceInfosResponseMessage, PairRequestMessage, RemovePairRequestMessage, RemovePairResponseMessage, ServerReadyResponseMessage, WorkErrorMessage};
use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};
use async_recursion::async_recursion;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Sender};
use tokio::sync::mpsc::Receiver;
use crate::handler::channel_manage::ChannelManage;

// pub struct PairChannelMap {
//     // channels: HashMap<String, Arc<Mutex<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>>>,
//     // channels: HashMap<String, (Sender<Vec<u8>>,i32)>,
//     channels: HashMap<String, (Sender<(String, SocketAddr)>, i32)>,
// }
// impl PairChannelMap {
//     pub(crate) fn new() -> Self {
//         Self {
//             channels: HashMap::with_capacity(40),
//         }
//     }
//     fn get_channel_by_id(
//         &mut self,
//         id: &str,
//         // ) -> Sender<Vec<u8>> {
//     ) -> Sender<(String, SocketAddr)> {
//         println!("进来寻找");
//         match self.channels.get(id) {
//             None => self.add_channel(id.into()),
//             Some(tuple) => (&tuple.0).clone(),
//         }
//     }
//     // fn add_channel(&mut self, id: String) -> Sender<Vec<u8>>{
//     fn add_channel(&mut self, id: String) -> Sender<(String, SocketAddr)> {
//         println!("进来添加通道");
//         let (tx, rx) = broadcast::channel(20);
//         let sender = tx.clone();
//         self.channels.insert(id, (tx, 1));
//         println!("添加通道完成");
//         sender
//     }
//     fn remove(&mut self, id: &str) {
//         if let Some(&mut (_, ref mut num)) = self.channels.get_mut(id) {
//             if *num > 1 {
//                 *num -= 1;
//             } else {
//                 // 如果num为1 即减1后每用户了，移除键值对，
//                 self.channels.remove(id);
//             }
//         };
//     }
// }
pub struct Context {
    // channel_map: Arc<RwLock<ChannelManage>>,
    channel_manage: Arc<RwLock<ChannelManage>>,
    // id: Option<String>,
    stream: TcpStream,
    socket_addr: SocketAddr,
    // 分配一个缓冲区
    buffer: BytesMut,
    // channel: Option<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>,
    // id: Option<String>,
    sender: Option<Sender<(String, SocketAddr)>>,
    pair_code:Option<String>,
    device_info:Option<DeviceInfo>
}
impl Context {
    // pub(crate) fn new(channel_map:Arc<ChannelMap>, tcp:TcpStream, socket:SocketAddr) ->Self{
    pub(crate) fn new(
        // channel_map: Arc<RwLock<ChannelManage>>,
        channel_manage: Arc<RwLock<ChannelManage>>,
        tcp: TcpStream,
        addr: SocketAddr,
    ) -> Self {
        Self {
            // channel_map,
            channel_manage,
            // id: None,
            stream: tcp,
            socket_addr: addr,
            buffer: BytesMut::with_capacity(512),
            sender: None,
            pair_code:None,
            device_info:None
        }
    }
    pub async fn send_ready(&mut self){
        //准备好接受消息了，给客户端发一个ready信号
        self.stream
            .write_all(ServerReadyResponseMessage().encode().as_slice())
            .await
            .unwrap();
    }
    ///开始监听该tcp的流消息（只关心配对相关请求），当收到消息后终止循环交由handle_before_pair_message函数处理
    /// 至于为什么不直接在循环中处理消息，考虑到如果配对成功，那么就直接进入新的loop，总感觉不好，并且会使start_work臃肿
    /// 改成监听到消息后终止循环去处理，如果配对失败，那么我们要恢复该流的监听状态，不然就直接结束了。
    /// 所有有可能再次调用start_work，形成递归。
    /// 所以如果有坏种疯狂乱发配对请求一直失败一直递归是不是会爆栈？
    /// 试了一下确实会爆栈（thread 'tokio-runtime-worker' has overflowed its stack），大概一直发了279个无效配对请求。
    #[async_recursion]
    pub async fn start_work(&mut self) {
        let message:Message = loop {
            println!("start_work 开始监听消息");
            let message = tokio::select! {
                msg = self.read_message() => msg
            };
            match message {
                Ok(om) => {
                    match om {
                        None => {
                            //说明对面Tcp关了
                            println!("对面断开连接");
                            self.disconnect();
                            break WorkErrorMessage();
                        }
                        Some(message) => {
                            break message;
                        }
                    }
                }
                Err(e) => {
                    //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                    println!("start_work error:{e}");
                    self.disconnect();
                    break WorkErrorMessage();
                }
            }
        };
        // println!("走出来了{:?}",message);
        self.handle_before_pair_message(message).await;
        // if id != "NULL".to_string() {
        //     self.set_id(id).await;
        // }
    }
    // async fn read_message(&mut self) ->Result<Option<Message>,String>{
    async fn read_message(&mut self) -> AppResult<Option<Message>> {
        loop {
            if let Some(message) = self.parse_message().await? {
                return Ok(Some(message));
            }
            //当 read 返回 Ok(0) 时，意味着字节流( stream )已经关闭，在这之后继续调用 read 会立刻完成，依然获取到返回值 Ok(0)。
            // 例如，字节流如果是 TcpStream 类型，那 Ok(0) 说明该连接的读取端已经被关闭(写入端关闭，会报其它的错误)。
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                return if self.buffer.is_empty() {
                    Ok(None)
                } else {
                    Ok(None)
                    // Err("connection reset by peer".into())
                };
            }
        }
    }
    async fn parse_message(&mut self) -> AppResult<Option<Message>> {
        // 创建 Cursor 类型 ，把当前缓冲区包起来
        let mut buf = Cursor::new(&self.buffer[..]);
        match Message::check_entire_message(&mut buf) {
            Ok(_) => {
                let x = buf.get_ref();
                let len = x.len();
                let message = Message::decode(x.to_vec())?;
                //将游标向后推len，圣经说是清空缓冲区的作用，我不理解
                self.buffer.advance(len);
                Ok(Some(message))
            }
            Err(IncompleteError) => {
                Ok(None)
                // Err("parse incomplete message".into())
            }
            Err(e) => {
                println!("这是什么error? {}", e.to_string());
                Err(e)
            } // _ => {}
        }
    }
    // async fn set_id(&mut self, id: String) {
    //     self.set_channel(&id).await;
    //     self.id = Some(id);
    // }
    async fn set_pair_channel(&mut self,tx:Sender<(String,SocketAddr)>){
        let mut rx = tx.subscribe();
        self.sender = Some(tx);
        let mut stream_rx = {
            let read_guard = self.channel_manage.read().unwrap();
            let stream_tx = read_guard.get_stream_channel(self.socket_addr).unwrap();
            stream_tx.subscribe()
        };
        loop {
            tokio::select! {
                message = rx.recv() => { //这个通道只用来接收剪切板消息
                    match message{
                        Ok((msg,addr)) => {
                            if self.socket_addr!= addr {
                                println!("管道收到{:?}的剪切板消息，准备发送给{:?}",addr,self.socket_addr);
                                self.send_message(ClipboardMessage(msg)).await;
                            }
                        }
                        Err(_) => {println!("receiver.recv()出现错误")}
                    }
                }
                message = stream_rx.recv() => { //这个用于接收其他tcpStream用管道发来的消息
                    match message{
                        Ok(message) => {
                            println!("tcp管道收到消息，准备处理哦");
                            // self.handle_before_pair_message(message).await;
                            self.handle_paired_message(message).await;
                        }
                        Err(_) => { println!("t receiver.recv()出现错误")}
                    }
                }
                message = self.read_message() =>{
                    match message {
                        Ok(om) => {
                            match om {
                                None => { //说明对面Tcp关了,我发现对面Ctrl+C直接终止进程和对面调用self.stream.shutdown()都走这个
                                    println!("对面断开连接");
                                    self.disconnect();
                                    break;
                                }
                                Some(message) => { //可能会收到：剪切板消息、移除设备消息
                                    println!("msg {:?}",message);
                                    self.handle_paired_message(message).await;
                                    // self.handle_after_message(message).await;
                                    // self.handle_message(message).await;
                                }
                            }
                        }
                        Err(e) => { //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                            println!("start_work error:{e}");
                            self.disconnect();
                            break;
                        }
                    }
                }
            }
        }
    }
    // async fn set_channel(&mut self, id: &str) {
    //     let mut receiver = {
    //         //这个作用域很关键，因为这里的Mutex锁用的是标准库的，不能跨await传递
    //         //所以必须在进入下面的tokio::select!前将锁释放
    //         // let arc_channel = self.channel_map.lock().unwrap().get_channel_by_id(&id);
    //         // let (sender, _) = &*arc_channel.lock().unwrap();
    //         // let new_sender = sender.clone();
    //         // let receiver = new_sender.subscribe();
    //         // self.sender = Some(new_sender);
    //         let sender = self.channel_map.lock().unwrap().get_channel_by_id(id);
    //         let receiver = sender.subscribe();
    //         self.sender = Some(sender);
    //         receiver
    //     };
    //     // self.channel = Some((new_sender,new_receiver));
    //     // self.channel = Some(self.channel_map.lock().unwrap().get_channel_by_id(&id));
    //     // self.channel = if let Some(asr) = self.channel_map.lock().unwrap().get_channel_by_id(&id){
    //     //     println!("找到了id{}的channel",id);
    //     //     let (sender,_) = &*asr.lock().unwrap();
    //     //     let new_sender = sender.clone();
    //     //     let new_receiver = new_sender.subscribe();
    //     //     Some((new_sender,new_receiver))
    //     //     // Some(asr)
    //     // }else {
    //     //     println!("没找到id{}的channel",id);
    //     //     Some(self.channel_map.lock().unwrap().add_channel(id))
    //     //     // Some(self.channel_map.add_channel(id))
    //     // };
    //     //有了channel后开始监听rec，这个channel应该只会接收ClipboardMessage，收到msg后发给对应的tcp让其设置自己的剪切板
    //     // let receiver = &mut self.channel.as_mut().unwrap().1;
    //     loop {
    //         tokio::select! {
    //             message = receiver.recv() => {
    //                 match message{
    //                     Ok((msg,addr)) => {
    //                         if self.socket_addr!=addr{
    //                             println!("{:?}收到消息，准备发送",self.stream);
    //                             &self.stream.write_all(ClipboardMessage(msg).encode().as_slice()).await;
    //                             &self.stream.flush().await;
    //                         }
    //                     }
    //                     Err(_) => {
    //                         println!("receiver.recv()出现错误")
    //                     }
    //                 }
    //             }
    //             message = self.read_message() =>{
    //                 match message {
    //                     Ok(om) => {
    //                         match om {
    //                             None => { //说明对面Tcp关了,我发现对面Ctrl+C直接终止进程和对面调用self.stream.shutdown()都走这个
    //                                 println!("对面断开连接");
    //                                 self.disconnect();
    //                                 return;
    //                             }
    //                             Some(message) => {
    //                                 println!("msg {:?}",message);
    //                                 match message{
    //                                     ClipboardMessage(content) => {
    //                                         self.handle_clipboard_message(content).await;
    //                                     }
    //                                     _ => {}
    //                                 }
    //                                 // self.handle_after_message(message).await;
    //                                 // self.handle_message(message).await;
    //                             }
    //                         }
    //                     }
    //                     Err(e) => { //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
    //                         println!("start_work error:{e}");
    //                         self.disconnect();
    //                         return;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     // let mut receiver = & mut (self.channel.as_mut().as_ref().unwrap().1);
    // }
    // async fn handle_pair_message(&mut self, id: String) {
    //     self.set_id(id).await;
    // }
    async fn handle_before_pair_message(&mut self,message: Message){
        match message {
            PairRequestMessage(pair_code, device) => {
                println!("{}收到请求配对消息{:?}",self.socket_addr,pair_code);
                let option = {
                    let mut channel_manage = self.channel_manage.write().unwrap();
                    channel_manage.get_pair_channel(&pair_code, &device)
                };
                match option {
                    None => {
                        println!("{}找不到配对设备",self.socket_addr);
                        self.send_message(NoPairDeviceResponseMessage()).await;
                        self.start_work().await;
                    }
                    Some((tx,vec)) => {
                        println!("{}配对成功",self.socket_addr);
                        //配对成功
                        self.send_message(PairDeviceInfosResponseMessage(vec)).await;
                        self.pair_code = Some(pair_code.into());
                        self.device_info = Some(device);
                        self.set_pair_channel(tx).await;
                    }
                }
            }
            PairCreateMessage(device) => {
                let (tx,pair_code) = {
                    let mut channel_manage = self.channel_manage.write().unwrap();
                    channel_manage.add_pair_channel(device.clone())
                };
                self.send_message(PairCodeResponseMessage(pair_code.clone())).await;
                self.pair_code = Some(pair_code);
                self.device_info = Some(device);
                self.set_pair_channel(tx).await;
            }
            _ => {}
        }
    }
    async fn handle_paired_message(&mut self,message: Message){
        match message {
            ClipboardMessage(content) => {
                println!("{}收到剪切板消息{}",self.socket_addr,content);
                if let Some(sender) = self.sender.as_ref() {
                    sender
                        .send((content,self.socket_addr))
                        .unwrap();
                }
            }
            RemovePairRequestMessage(addr) => {
                println!("{}收到断开{}连接的请求",self.socket_addr,addr);
                //这个断开连接的请求有可能是自己发来的,也有可能是通过管子发来的，有可能是要求断开自己的，也有可能是别人
                if addr==self.socket_addr { //如果请求自己断开连接，那就别转发给别的管子了
                    self.clear_pair_state();
                    // self.disconnect();
                    self.send_message(RemovePairResponseMessage()).await;
                    //清除配对状态后不是断开连接了，要继续等待配对请求！！
                    self.start_work().await;
                }else {//请求断开别人
                    let read_guard = self.channel_manage.read().unwrap();
                    if let Some(tx) = read_guard.get_stream_channel(addr) {
                        // tx.send(RemovePairResponseMessage()).unwrap();
                        tx.send(RemovePairRequestMessage(addr)).unwrap();
                    }
                }
            }
            _ => {}
        }
    }
    // async fn handle_clipboard_message(&mut self,content:Vec<u8>){
    // async fn handle_clipboard_message(&mut self, content: String) {
    //     if let Some(sender) = self.sender.as_ref() {
    //         sender
    //             // .send(ClipboardMessage(content).encode_message())
    //             .send((content,self.socket_addr))
    //             .unwrap();
    //     }
    // }
    // async fn handle_before_message(&mut self, message: Message) {
    //     match message {
    //         PairRequestMessage(id) => {
    //             if self.sender.is_none() {
    //                 self.set_channel("123456".into()).await;
    //             }
    //         }
    //         _ => {}
    //     }
    // }
    // async fn handle_after_message(&mut self, message: Message) {
    //     match message {
    //         ClipboardMessage(content) => {
    //             // println!("收到了消息ClipboardMessage{:?}",content);
    //             //todo 有点脱裤子放屁了,先从ClipboardMessage结构又构建回来了
    //             if let Some(sender) = self.sender.as_ref() {
    //                 sender
    //                     .send(ClipboardMessage(content).encode_message())
    //                     .unwrap();
    //             }
    //         }
    //         _ => {}
    //     }
    // }
    // async fn handle_message(&mut self, message: Message) {
    //     match message {
    //         PairRequestMessage(username) => {
    //             //todo 验证账号密码是否正确，若正确，取回id
    //             if self.sender.is_none() {
    //                 self.set_channel("123456".into()).await;
    //             }
    //         }
    //         ClipboardMessage(content) => {
    //             // println!("收到了消息ClipboardMessage{:?}",content);
    //             //todo 有点脱裤子放屁了,先从ClipboardMessage结构又构建回来了
    //             if let Some(sender) = self.sender.as_ref() {
    //                 sender
    //                     .send(ClipboardMessage(content).encode_message())
    //                     .unwrap();
    //             }
    //             // if let Some(asr) = self.channel.as_ref(){
    //             //     *asr.lock().unwrap().0.send(Serializer::encode_message(ClipboardMessage(content)))
    //             // }
    //         }
    //         _ => {}
    //     }
    // }
    async fn send_message(&mut self, message: Message){
        self.stream.write_all(message.encode().as_slice()).await.unwrap();
        self.stream.flush().await.unwrap();
    }
    ///清除配对状态要处理：①移除tcp_channel ②移除pair_channel
    /// ③移除配对码 设备信息应该不用移除，不过移不移除都一样了，反正会重新设置
    fn clear_pair_state(&mut self){
        println!("{}清除连接状态",self.socket_addr);
        let mut write_guard = self.channel_manage.write().unwrap();
        if let (Some(pair_code),Some(device_info)) =  (&self.pair_code,&self.device_info) {
            write_guard.remove_pair_device(pair_code,device_info.clone());
        }
        self.pair_code = None;
    }
    fn disconnect(&mut self) {
        // println!("{}清除连接状态",self.socket_addr);
        // let mut write_guard = self.channel_manage.write().unwrap();
        // if let (Some(pair_code),Some(device_info)) =  (&self.pair_code,&self.device_info) {
        //     write_guard.remove_pair_device(pair_code,device_info.clone());
        // }
        println!("{}断开连接",self.socket_addr);
        self.clear_pair_state();
        let mut write_guard = self.channel_manage.write().unwrap();
        write_guard.remove_stream_channel(self.socket_addr);
        // self.pair_code = None;
        // write_guard.remove_pair_device()
        // if let Some(id) = &self.id {
        //     let mut g_channel_map = self.channel_map.lock().unwrap();
        //     g_channel_map.remove(id);
        // }
    }
}
