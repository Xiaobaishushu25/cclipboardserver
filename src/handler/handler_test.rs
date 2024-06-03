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

pub struct ContextTest {
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
    check_connect: (Option<DelayTimer>, Sender<Message>), // check_connectOption<>
}
impl ContextTest {
    pub(crate) fn new(
        channel_manage: Arc<RwLock<ChannelManage>>,
        tcp: TcpStream,
        addr: SocketAddr,
    ) -> Self {
        // let (read,write) = tcp.into_split();
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
            // check_connect: (delay_timer, tx),
            check_connect: (None, tx),
        }
    }
}