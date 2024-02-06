use crate::app_errors::AppError::IncompleteError;
use crate::app_errors::AppResult;
use crate::message::message::Message;
use crate::message::message::Message::{ClipboardMessage, LoginRequestMessage, ServerReadyMessage};
use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

// trait Handler {
//     fn handle(msg: Message);
//     fn set_next_handler<T>(handler: T) -> T
//     where
//         T: Handler;
// }
// struct HandlerPipe{
//
// }
//
// impl HandlerPipe {
//
// }
// struct LoginRequestHandler;
//
// impl Handler for LoginRequestHandler {
//     fn handle(msg: Message) {
//         if let Message::LoginRequestMessage(_, _) = msg {};
//     }
//
//     fn set_next_handler<T>(handler: T) -> T where T: Handler {
//         todo!()
//     }
// }
pub struct ChannelMap {
    //现在的channel没有用户信息数量，应当记录当前使用者，没有使用者要清除这个channel
    channels: HashMap<String, Arc<Mutex<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>>>,
}

impl ChannelMap {
    pub(crate) fn new() -> Self {
        Self {
            channels: HashMap::with_capacity(100),
        }
    }
    fn get_channel_by_id(
        &mut self,
        id: &String,
    ) -> Arc<Mutex<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>> {
        println!("进来寻找");
        match self.channels.get(id) {
            None => self.add_channel(id),
            Some(asr) => {
                println!("找到了");
                asr.clone()
            }
        }
    }
    fn add_channel(&mut self, id: &String) -> Arc<Mutex<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>> {
        println!("进来添加通道");
        let abc = Arc::new(Mutex::new(broadcast::channel(20)));
        // let (sender,_) = &*abc.lock().unwrap();
        // let sender = sender.clone();
        // let receiver = sender.subscribe();
        self.channels.insert(id.clone(), abc.clone());
        println!("添加通道完成");
        abc
        // (sender,receiver)
    }
    // fn add_channel(&mut self,id:&String)->(Sender<Vec<u8>>, Receiver<Vec<u8>>){
    //     println!("进来添加通道");
    //     let abc= Arc::new(Mutex::new(broadcast::channel(20)));
    //     let (sender,_) = &*abc.lock().unwrap();
    //     let sender = sender.clone();
    //     let receiver = sender.subscribe();
    //     self.channels.insert(id.clone(),abc.clone());
    //     println!("添加通道完成");
    //     (sender,receiver)
    // }
}
pub struct Context {
    // channel_map:Arc<ChannelMap>,
    channel_map: Arc<Mutex<ChannelMap>>,
    id: Option<String>,
    stream: TcpStream,
    socket: SocketAddr,
    // 分配一个缓冲区
    buffer: BytesMut,
    // channel: Option<(Sender<Vec<u8>>, Receiver<Vec<u8>>)>,
    sender: Option<Sender<Vec<u8>>>,
}
impl Context {
    // pub(crate) fn new(channel_map:Arc<ChannelMap>, tcp:TcpStream, socket:SocketAddr) ->Self{
    pub(crate) fn new(
        channel_map: Arc<Mutex<ChannelMap>>,
        tcp: TcpStream,
        socket: SocketAddr,
    ) -> Self {
        Self {
            channel_map,
            id: None,
            stream: tcp,
            socket,
            buffer: BytesMut::with_capacity(512),
            sender: None,
        }
    }
    pub async fn start_work(&mut self) {
        self.stream
            .write_all(ServerReadyMessage(true).encode_message().as_slice())
            .await
            .unwrap();

        // let mut buffer:[u8;10] = [0;10];
        loop {
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
                            return;
                        }
                        Some(message) => {
                            println!("msg {:?}", message);
                            self.handle_before_message(message).await;
                            // self.handle_message(message).await;
                        }
                    }
                }
                Err(e) => {
                    //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                    println!("start_work error:{e}");
                    return;
                }
            }
        }
        //发一个serverready信号
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
                let message = Message::decode_message(x.to_vec())?;
                //将游标向后推len，圣经说是清空缓冲区的作用，我不理解
                self.buffer.advance(len);
                Ok(Some(message))
            }
            Err(IncompleteError) => {
                Ok(None)
                // Err("parse incomplete message".into())
            }
            Err(e) => {
                println!("这是什么error？ {}", e.to_string());
                Err(e)
            } // _ => {}
        }
    }
    // fn set_id(&mut self, id:String){
    //     self.id = Some(id)
    // }
    async fn set_channel(&mut self, id: String) {
        let mut new_receiver = {
            //这个作用域很关键，因为这里的Mutex锁用的是标准库的，不能跨await传递
            //所以必须在进入下面的tokio::select!前将锁释放
            let arc_channel = self.channel_map.lock().unwrap().get_channel_by_id(&id);
            let (sender, _) = &*arc_channel.lock().unwrap();
            let new_sender = sender.clone();
            let receiver = new_sender.subscribe();
            self.sender = Some(new_sender);
            receiver
        };
        // self.channel = Some((new_sender,new_receiver));
        // self.channel = Some(self.channel_map.lock().unwrap().get_channel_by_id(&id));
        // self.channel = if let Some(asr) = self.channel_map.lock().unwrap().get_channel_by_id(&id){
        //     println!("找到了id{}的channel",id);
        //     let (sender,_) = &*asr.lock().unwrap();
        //     let new_sender = sender.clone();
        //     let new_receiver = new_sender.subscribe();
        //     Some((new_sender,new_receiver))
        //     // Some(asr)
        // }else {
        //     println!("没找到id{}的channel",id);
        //     Some(self.channel_map.lock().unwrap().add_channel(id))
        //     // Some(self.channel_map.add_channel(id))
        // };
        println!("获取rec监听通道消息");
        //有了channel后开始监听rec，这个channel应该只会接收ClipboardMessage，收到msg后发给对应的tcp让其设置自己的剪切板
        // let receiver = &mut self.channel.as_mut().unwrap().1;
        // let mut receiver = & mut (self.channel.as_mut().as_ref().unwrap().1);
        loop {
            tokio::select! {
                c = new_receiver.recv() => {
                    match c{
                        Ok(msg) => {
                            println!("{:?}收到消息，准备发送",self.stream);
                            &self.stream.write_all(msg.as_slice()).await;
                            &self.stream.flush().await;
                        }
                        Err(_) => {
                            println!("receiver.recv()出现错误")
                        }
                    }
                }
                message = self.read_message() =>{
                    match message {
                        Ok(om) => {
                            match om {
                                None => { //说明对面Tcp关了
                                    println!("对面断开连接");
                                    return;
                                }
                                Some(message) => {
                                    println!("msg {:?}",message);
                                    self.handle_after_message(message).await;
                                    // self.handle_message(message).await;
                                }
                            }
                        }
                        Err(e) => { //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                            println!("start_work error:{e}");
                            return;
                        }
                    }
                }
            }
            // println!("开始监听通道消息");
            // match new_receiver.recv().await{
            //     Ok(msg) => {
            //         println!("{:?}收到消息，准备发送",self.stream);
            //         &self.stream.write_all(msg.as_slice());
            //     }
            //     Err(_) => {
            //         println!("receiver.recv()出现错误")
            //     }
            // }
        }
    }
    async fn handle_before_message(&mut self, message: Message) {
        match message {
            LoginRequestMessage(username) => {
                //todo 验证账号密码是否正确，若正确，取回id
                if self.sender.is_none() {
                    self.set_channel("123456".into()).await;
                }
            }
            _ => {}
        }
    }
    async fn handle_after_message(&mut self, message: Message) {
        match message {
            ClipboardMessage(content) => {
                // println!("收到了消息ClipboardMessage{:?}",content);
                //todo 有点脱裤子放屁了,先从ClipboardMessage结构又构建回来了
                if let Some(sender) = self.sender.as_ref() {
                    sender
                        .send(ClipboardMessage(content).encode_message())
                        .unwrap();
                }
            }
            _ => {}
        }
    }
    async fn handle_message(&mut self, message: Message) {
        match message {
            LoginRequestMessage(username) => {
                //todo 验证账号密码是否正确，若正确，取回id
                if self.sender.is_none() {
                    self.set_channel("123456".into()).await;
                }
            }
            ClipboardMessage(content) => {
                // println!("收到了消息ClipboardMessage{:?}",content);
                //todo 有点脱裤子放屁了,先从ClipboardMessage结构又构建回来了
                if let Some(sender) = self.sender.as_ref() {
                    sender
                        .send(ClipboardMessage(content).encode_message())
                        .unwrap();
                }
                // if let Some(asr) = self.channel.as_ref(){
                //     *asr.lock().unwrap().0.send(Serializer::encode_message(ClipboardMessage(content)))
                // }
            }
            _ => {}
        }
    }
}
