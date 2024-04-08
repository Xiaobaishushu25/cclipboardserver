use crate::app_errors::AppError::{AnyHow, ErrorDescribe, IncompleteError};
use crate::app_errors::{AppError, AppResult};
use crate::handler::channel_manage::ChannelManage;
use crate::message::message::Message::{
    ClipboardMessage, CloseMessage, DeviceChangeResponseMessage, HeartPackageMessage,
    NoPairDeviceResponseMessage, PairCodeResponseMessage, PairCreateMessage,
    PairDeviceInfosResponseMessage, PairRequestMessage, RemovePairRequestMessage,
    RemovePairResponseMessage, ServerReadyResponseMessage, WorkErrorMessage,
};
use crate::message::message::{DeviceInfo, Message};
use async_recursion::async_recursion;
use bytes::{Buf, BytesMut};
use delay_timer::prelude::{DelayTimer, DelayTimerBuilder, Task, TaskBuilder};
use log::{debug, error, info};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use anyhow::anyhow;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

pub struct Context {
    count: i32, //尝试次数，若重新进入start_work函数次数过多（200）强制断开连接，防止爆栈
    // channel_map: Arc<RwLock<ChannelManage>>,
    channel_manage: Arc<RwLock<ChannelManage>>,
    stream: TcpStream,
    socket_addr: SocketAddr,
    // 分配一个缓冲区
    buffer: BytesMut,
    // sender: Option<Sender<(String, SocketAddr)>>,
    //配对通道
    pair_tx: Option<Sender<(Message, SocketAddr)>>,
    pair_code: Option<String>,
    device_info: Option<DeviceInfo>,
    //存活检查，第一个是一个定时器，第二个用于发送断开的消息
    check_connect: (DelayTimer, Sender<Message>), // check_connectOption<>
}
impl Context {
    // pub(crate) fn new(channel_map:Arc<ChannelMap>, tcp:TcpStream, socket:SocketAddr) ->Self{
    pub(crate) fn new(
        channel_manage: Arc<RwLock<ChannelManage>>,
        tcp: TcpStream,
        addr: SocketAddr,
    ) -> Self {
        let delay_timer = DelayTimerBuilder::default().build();
        let (tx, _) = broadcast::channel(16);
        Self {
            count: 0,
            channel_manage,
            stream: tcp,
            socket_addr: addr,
            buffer: BytesMut::with_capacity(512),
            pair_tx: None,
            pair_code: None,
            device_info: None,
            check_connect: (delay_timer, tx),
        }
    }
    pub async fn send_ready(&mut self) {
        //开启定时任务
        let timer = &self.check_connect.0;
        timer.add_task(self.build_clear_task()).unwrap();
        //准备好接受消息了，给客户端发一个ready信号
        let response_message = ServerReadyResponseMessage(self.socket_addr);
        info!("send message:{:?} to {}", response_message, self.socket_addr);
        self.send_socket_message(response_message).await;
        // self.stream
        //     .write_all(response_message.encode().as_ref())
        //     .await
        //     .unwrap();
    }
    ///开始监听该tcp的流消息（只关心配对相关请求），当收到消息后终止循环交由handle_before_pair_message函数处理
    /// 至于为什么不直接在循环中处理消息，考虑到如果配对成功，那么就直接进入新的loop，感觉这个函数没结束干净，总感觉不好，并且会使start_work臃肿
    /// 改成监听到消息后终止循环去处理，如果配对失败，那么我们要恢复该流的监听状态，不然就直接结束了。
    /// 所以有可能再次调用start_work，形成递归。
    /// 所以如果有坏种疯狂乱发配对请求一直失败一直递归是不是会爆栈？
    /// 试了一下确实会爆栈（thread 'tokio-runtime-worker' has overflowed its stack），大概一直发了279个无效配对请求。
    #[async_recursion]
    pub async fn start_work(&mut self) {
        self.count += 1;
        if self.count > 200 {
            error!("{} too many attempts", self.socket_addr);
            self.disconnect().await;
        }
        info!("start to listen {} message", self.socket_addr);
        let connect_tx = self.check_connect.1.clone();
        let mut connect_rx = connect_tx.subscribe();
        let message: Message = loop {
            let message = tokio::select! {
                msg = self.read_message() => msg,
                msg = connect_rx.recv() => {
                    Ok(Some(WorkErrorMessage()))
                }
            };
            match message {
                Ok(om) => {
                    match om {
                        None => {
                            //说明对面Tcp关了
                            error!("message is None");
                            // self.disconnect().await;
                            break WorkErrorMessage();
                        }
                        Some(message) => {
                            //注意，这里要单独处理心跳包，而set_pair_channel函数则不用，因为这里收到消息包后会break
                            //如果配对失败还会递归进来，如果收到心跳包就break那么分分钟爆栈。
                            if let HeartPackageMessage() = message {
                                // println!("收到一个心跳包");
                                self.send_socket_message(HeartPackageMessage()).await;
                            } else if let ClipboardMessage(_) = message {
                                //如果他没配对但是打开软件还一直复制的话，不单独处理也会一直递归进来
                            } else if let RemovePairRequestMessage(_) = message {
                                //
                            } else {
                                info!("listen:{:?} from {}", message, self.socket_addr);
                                break message;
                            }
                            // break message;
                        }
                    }
                }
                Err(e) => {
                    //发送错误的数据包也会走到这里
                    //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                    error!("{e}");
                    // self.disconnect().await;
                    break WorkErrorMessage();
                }
            }
        };
        // info!("走出来了{:?}",message);
        self.handle_before_pair_message(message).await;
    }
    ///配对成功后调用此函数，开始循环监听三个事件：剪切板管道事件、tcp通知管道事件、tcp流通知事件。
    // async fn set_pair_channel(&mut self,tx:Sender<(String,SocketAddr)>){
    async fn set_pair_channel(&mut self, tx: Sender<(Message, SocketAddr)>) {
        let mut pair_rx = tx.subscribe();
        self.pair_tx = Some(tx);
        let mut stream_rx = {
            let read_guard = self.channel_manage.read().unwrap();
            let stream_tx = read_guard.get_stream_channel(self.socket_addr).unwrap();
            stream_tx.subscribe()
        };
        let connect_tx = self.check_connect.1.clone();
        let mut connect_rx = connect_tx.subscribe();
        loop {
            tokio::select! {
                message = pair_rx.recv() => { //这个通道只用来接收配对群消息  剪切板消息
                    match message{
                        Ok((msg,addr)) => {
                            if self.socket_addr!= addr {
                                debug!("{}广播管道收到{}的剪切板消息.",addr,self.socket_addr);
                                match msg{
                                    ClipboardMessage(_)|
                                    DeviceChangeResponseMessage(_,_) =>{
                                        self.send_socket_message(msg).await;
                                    }
                                    _ => {}
                                }
                                // debug!("{}广播管道收到{}的剪切板消息.",addr,self.socket_addr);
                                // self.send_message(ClipboardMessage(msg)).await;
                            }
                        }
                        Err(e) => {error!("rx.recv()出现错误:{}",e.to_string())}
                    }
                }
                message = stream_rx.recv() => { //这个用于接收其他tcpStream用管道发来的消息
                    match message{
                        Ok(message) => {
                            debug!("{}的socket管道收到消息：{:?}",self.socket_addr,message);
                            // self.handle_before_pair_message(message).await;
                            self.handle_paired_message(message).await;
                        }
                        Err(e) => {
                            //这个错误在客户端关闭后稳定会出现一次的，因为处理完关闭请求还会回到这个循环
                            //但是此时发送端已被清理掉，所以会报错，正常，进入这里直接break就好了。
                            error!("stream_rx.recv()出现错误:{}",e.to_string());
                            break;
                        }
                    }
                }
                message = self.read_message() =>{
                    match message {
                        Ok(om) => {
                            match om {
                                None => { //说明对面Tcp关了,我发现对面Ctrl+C直接终止进程和对面调用self.stream.shutdown()都走这个
                                    // info!("对面断开连接");
                                    error!("message is None");
                                    self.disconnect().await;
                                    break;
                                }
                                Some(message) => { //可能会收到：心跳包、剪切板消息、移除设备消息
                                    // info!("msg {:?}",message);
                                    // if let HeartPackageMessage() = message{
                                    //     println!("收到一个心跳包");
                                    //     self.send_message(HeartPackageMessage())
                                    // }else {
                                    //     self.handle_paired_message(message).await;
                                    // }
                                    self.handle_paired_message(message).await;
                                }
                            }
                        }
                        Err(e) => { //对面被强制关闭时会走这里start_work error:io::Error:`远程主机强迫关闭了一个现有的连接。 (os error 10054)`
                            error!("{e}");
                            self.disconnect().await;
                            break;
                        }
                    }
                }
                message = connect_rx.recv() => {
                    error!("{} receive WorkErrorMessage,disconnect",self.socket_addr);
                    self.disconnect().await;
                    break;
                }
            }
        }
    }

    ///处理配对成功前的消息，只关心两种消息：请求配对消息和请求创建配对消息,记得没配对成功要重新开始监听
    async fn handle_before_pair_message(&mut self, message: Message) {
        match message {
            PairRequestMessage(pair_code, device) => {
                // info!("{}收到请求配对消息{:?}",self.socket_addr,pair_code);
                let option = {
                    let mut channel_manage = self.channel_manage.write().unwrap();
                    channel_manage.get_pair_channel(&pair_code, &device)
                };
                match option {
                    None => {
                        info!(
                            "{} cant find pair device by {:?}",
                            self.socket_addr, pair_code
                        );
                        self.send_socket_message(NoPairDeviceResponseMessage())
                            .await;
                        self.start_work().await;
                    }
                    Some((tx, vec)) => {
                        info!("{} pair success by {:?}", self.socket_addr, pair_code);
                        self.send_socket_message(PairDeviceInfosResponseMessage(vec))
                            .await;
                        self.pair_code = Some(pair_code.into());
                        self.device_info = Some(device.clone());
                        //配对成功,给这个通道的设备发一个deice change消息
                        let tmp_tx = tx.clone();
                        tmp_tx
                            .send((DeviceChangeResponseMessage(true, device), self.socket_addr))
                            .unwrap();
                        self.set_pair_channel(tx).await;
                    }
                }
            }
            PairCreateMessage(device) => {
                let (tx, pair_code) = {
                    let mut channel_manage = self.channel_manage.write().unwrap();
                    info!(
                        "now pair group num is {}",
                        channel_manage.get_pair_num() + 1
                    );
                    channel_manage.add_pair_channel(device.clone())
                };
                self.send_socket_message(PairCodeResponseMessage(pair_code.clone()))
                    .await;
                self.pair_code = Some(pair_code);
                self.device_info = Some(device);
                self.set_pair_channel(tx).await;
            }
            CloseMessage() => {
                self.disconnect().await;
            }
            WorkErrorMessage() => {
                self.disconnect().await;
            }
            _ => {
                self.start_work().await;
            }
        }
    }
    ///处理配对成功后的消息
    async fn handle_paired_message(&mut self, message: Message) {
        if let HeartPackageMessage() = message {
            //心跳包就别打印了
        } else {
            info!("accept:{:?} from {}", message, self.socket_addr);
        }
        match message {
            HeartPackageMessage() => {
                self.send_socket_message(HeartPackageMessage()).await;
            }
            ClipboardMessage(_) => {
                self.broadcast_message(message);
            }
            RemovePairRequestMessage(addr) => {
                // info!("收到{}断开{}连接的请求",self.socket_addr,addr);
                //这个断开连接的请求有可能是自己发来的,也有可能是通过管子发来的，有可能是要求断开自己的，也有可能是别人
                if addr == self.socket_addr {
                    //如果请求自己断开连接，那就别转发给别的管子了
                    // let device_info = self.device_info.as_ref().unwrap();
                    // self.broadcast_message(DeviceChangeResponseMessage(false, device_info.clone()));
                    self.clear_pair_state();
                    // self.disconnect().await;
                    self.send_socket_message(RemovePairResponseMessage()).await;
                    //清除配对状态后不是断开连接了，要继续等待配对请求！！
                    self.start_work().await;
                } else {
                    //请求断开别人
                    let read_guard = self.channel_manage.read().unwrap();
                    if let Some(tx) = read_guard.get_stream_channel(addr) {
                        tx.send(RemovePairRequestMessage(addr)).unwrap();
                    }
                }
            }
            CloseMessage() => {
                self.disconnect().await;
            }
            _ => {}
        }
    }
    ///将消息广播至配对群
    #[inline]
    fn broadcast_message(&mut self, message: Message) {
        info!("{} broadcast {:?}", self.socket_addr, message);
        if let Some(sender) = self.pair_tx.as_ref() {
            sender
                // .send((content,self.socket_addr))
                .send((message, self.socket_addr))
                .unwrap();
        }
    }

    ///将消息写入tcp流发送,并清除定时任务再新建一个定时任务
    async fn send_socket_message(&mut self, message: Message) {
        if let HeartPackageMessage() = message {
            //心跳包就别打印了
        } else {
            info!("send tcp：{:?} to {}", message, self.socket_addr);
        }
        //called `Result::unwrap()` on an `Err` value: Os { code: 104, kind: ConnectionReset, message: "Connection reset by peer" }
        let _ = self.stream
            .write_all(message.encode().as_ref())
            .await;
        let _ = self.stream.flush().await;
        let timer = &self.check_connect.0;
        timer.remove_task(1).unwrap();
        timer.add_task(self.build_clear_task()).unwrap()
    }
    async fn read_message(&mut self) -> AppResult<Option<Message>> {
        // info!("开始read_message");
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
                    // Err(AppError::from(anyhow!("connection reset by peer")))
                    Err(AnyHow(anyhow!("connection reset by peer")))
                    // anyhow!("connection reset by peer".into())
                    // Ok(None)
                    // Err(ErrorDescribe("connection reset by peer".into()))
                };
            }
        }
    }
    async fn parse_message(&mut self) -> AppResult<Option<Message>> {
        // 创建 Cursor 类型 ，把当前缓冲区包起来(模仿的mini-redis)
        let mut buf = Cursor::new(&self.buffer[..]);
        //检查当前数据是否可以解析出一个完整的Message
        match Message::check_entire_message(&mut buf) {
            Ok(_) => {
                //是一个完整的消息，记录当前数据长度
                let len = buf.position();
                //check_entire_message中调了游标位置，要调回来，下面解码要用
                buf.set_position(0);
                // let x = buf.get_ref();
                // let len = x.len();
                // let message = Message::decode(x.to_vec())?;
                let message = Message::decode(&mut buf)?;
                //将游标向后推len，圣经说是清空缓冲区的作用，我不理解
                self.buffer.advance(len as usize);
                //advance完内容长度变为0了，具体为什么还不知道
                Ok(Some(message))
            }
            Err(IncompleteError) => {
                Ok(None)
                // Err("parse incomplete message".into())
            }
            Err(e) => {
                info!("what this error? {}", e.to_string());
                Err(e)
            } // _ => {}
        }
    }
    ///构建一个清除任务，id为1，60秒后发送WorkErrorMessage，使该socket断开连接。
    fn build_clear_task(&self) -> Task {
        let connect_tx = self.check_connect.1.clone();
        let addr = self.socket_addr.clone();
        let mut builder = TaskBuilder::default();
        let task = move || {
            let new_tx = connect_tx.clone();
            async move {
                error!("{} timer execute! send WorkErrorMessage!",addr);
                //不要unwrap，因为有可能已经清除掉连接了，在unwrap会panic
                let _ = new_tx.send(WorkErrorMessage());
            }
        };
        builder
            .set_task_id(1)
            .set_frequency_once_by_seconds(60)
            .spawn_async_routine(task)
            .unwrap()
    }
    ///清除配对状态要处理 ①移除pair_channel
    /// ②移除配对信息并广播  设备信息应该不用移除，不过移不移除都一样了，反正会重新设置
    fn clear_pair_state(&mut self) {
        info!("clear {} pair state", self.socket_addr);
        //有可能客户端没有配对上就断开连接了，先检查一下是否配对在广播
        if let (Some(info), Some(_)) = (&self.device_info, &self.pair_code) {
            self.broadcast_message(DeviceChangeResponseMessage(false, info.clone()));
        }
        let mut write_guard = self.channel_manage.write().unwrap();
        if let (Some(pair_code), Some(device_info)) = (&self.pair_code, &self.device_info) {
            write_guard.remove_pair_device(pair_code, device_info.clone());
        }
        self.pair_code = None;
    }
    ///关闭stream 清理tcp_channel
    async fn disconnect(&mut self) {
        info!("{} is disconnect", self.socket_addr);
        self.clear_pair_state();
        let mut write_guard = self.channel_manage.write().unwrap();
        write_guard.remove_stream_channel(self.socket_addr);
        //本来想关闭连接的，但是报错：error: future cannot be sent between threads safely
        //#[async_recursion]
        //^^^^^^^^^^^^^^^^^^ future created by async block is not `Send`
        // let _ = self.stream.shutdown().await;
    }
}
