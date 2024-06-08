use crate::app_errors::AppError::{IncompleteError, MessageFormatError};
use crate::app_errors::AppResult;
use bytes::{Buf, BufMut, BytesMut};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Seek, SeekFrom};
use std::net::{SocketAddr};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    //这个地址用于  判断断开请求的是否由自己stream发送（不过也可以根据设备信息判断）
    //必要用途：某设备请求断开其他设备的配对，此时需要根据这个地址转发请求给对应的tcpStream
    socket_addr: SocketAddr, //这个地址用于在判断是否断开请求的是不是自己stream请求断开的不是本机的是
    // device_id:String, //这个地址用于在判断是否断开请求的是不是自己stream请求断开的不是本机的是
    device_name: String,
    device_type: DeviceType,
    // pair_code:Option<String>
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeviceType {
    DeskTop,
    Phone,
}
#[warn(dead_code)]
impl DeviceInfo {
    pub(crate) fn new(socket_addr: SocketAddr, device_name: String) -> Self {
        Self {
            socket_addr,
            device_name,
            device_type: DeviceType::DeskTop,
        }
    }
    pub fn get_addr(&self) -> SocketAddr {
        self.socket_addr
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    //心跳包
    HeartPackageMessage(),
    //六位随机数配对码
    //将自己设备信息和配对码发送给请求配对
    //配对码，设备信息，是否在不存在时创建
    PairRequestMessage(String, DeviceInfo,bool),
    //将自己设备信息发送给请求服务器创建一个配对码
    PairCreateMessage(DeviceInfo),
    PairDeviceInfosResponseMessage(Vec<DeviceInfo>),
    //用于服务器返回配对码给客户端
    PairCodeResponseMessage(String),
    //剪切板内容content
    ClipboardMessage(String),
    //服务端准备好后会返回一个含有SocketAddr的ready信息
    // ServerReadyResponseMessage(),
    ServerReadyResponseMessage(SocketAddr),
    //没有配对的设备
    NoPairDeviceResponseMessage(),
    //请求服务端移除配对通道的指定的设备
    RemovePairRequestMessage(SocketAddr),
    // RemovePairRequestMessage(String),
    //被移除配对通知
    RemovePairResponseMessage(),
    //工作失败错误，用于start_work跳出循环用
    WorkErrorMessage(),
    //设备变化信息，true代表有设备加入配对，false代表该设备断开配对
    DeviceChangeResponseMessage(bool, DeviceInfo),
    //客户端关闭
    CloseMessage(),
}
impl Message {
    pub fn get_tye_id(&self) -> u8 {
        match self {
            Message::HeartPackageMessage() => 0,
            // Message::PairRequestMessage(_, _) => 1,
            Message::PairRequestMessage(_,_, _) => 1,
            Message::PairCreateMessage(_) => 2,
            Message::PairDeviceInfosResponseMessage(_) => 3,
            Message::PairCodeResponseMessage(_) => 4,
            Message::ClipboardMessage(_) => 5,
            Message::ServerReadyResponseMessage(_) => 6,
            Message::NoPairDeviceResponseMessage() => 7,
            Message::RemovePairRequestMessage(_) => 8,
            Message::RemovePairResponseMessage() => 9,
            Message::WorkErrorMessage() => 10,
            Message::DeviceChangeResponseMessage(_, _) => 11,
            Message::CloseMessage() => 12,
        }
    }
    ///检查src是否是一个完整的message，根据协议，第一个字节是消息类型，接下来的四个字节（也就是一个i32）是消息体长度
    /// 剩余应该全是消息内容
    pub fn check_entire_message(src: &mut Cursor<&[u8]>) -> AppResult<()> {
        if !src.has_remaining() {
            Err(IncompleteError)
        } else {
            //src.get_u8()获取第一个u8，并将游标向后推一位
            // let size = src.get_u8();
            // let remain_size = src.get_ref().len() - 1;
            //如果src中的数据量连类型和长度声明的大小都不足，则先跳过等待后面更多数据的到来
            if src.remaining() < 5 {
                Err(IncompleteError)
            } else {
                //src.get_u8()获取第一个u8(类型id)，自动将游标向后推一位
                let _ = src.get_u8();
                //src.get_i32()获取接下来的4个字节(内容长度)，自动将游标向后推4位
                let len = src.get_i32();
                // info!("规定长度是{}",len);
                //缓冲区剩余的bytes数量（between the current position and the end of the buffer）
                let remain_size = src.remaining() as i32;
                // info!("缓冲区剩余长度是{}",remain_size);
                // if size == (remain_size as u8) {
                if len <= remain_size {
                    //等于的时候刚好是一个完整的消息，大于的时候说明有粘包
                    //把游标位置调到一个完整的消息处，用于后续清空该消息的缓冲
                    //如果 len 是负数或者 len 转换为 i64 后超出了 i32 的范围，那么 seek 操作会失败，因为 Cursor 无法移动到一个无效的位置。
                    // 这可能是由于数据损坏、错误的数据格式或者网络问题导致的。
                    // src.seek(SeekFrom::Current(len as i64)).unwrap();
                    // Ok(())
                    if let Ok(_) = src.seek(SeekFrom::Current(len as i64)){
                        Ok(())
                    }else {
                        Err(MessageFormatError)
                    }

                } else {
                    Err(IncompleteError)
                }
            }
        }
    }
    ///编码消息，转移所有权，
    /// 编码方式为：第一个字节是消息类型，接下来的四个字节（也就是一个i32）是消息体长度，把内容加在后面
    // pub fn encode(self) -> Vec<u8> {
    pub fn encode(self) -> BytesMut {
        // let mut bytes = Vec::with_capacity(256);
        let content = serde_json::to_vec(&self).unwrap();
        // let len = content.len() as i32;
        let mut bytes_mut = BytesMut::with_capacity(content.len() + 1 + 4);
        bytes_mut.put_u8(self.get_tye_id());
        bytes_mut.put_i32(content.len() as i32);
        bytes_mut.extend_from_slice(content.as_slice());
        bytes_mut
        // println!("内容的长度是{}  转成u8是{}",content.len(),content.len() as u8);
        // bytes.push(content.len() as u8); //第一部分放一个u8，代表消息长度
        // bytes.extend(content); //第二部分代表内容
        // bytes
    }
    ///解码方式：忽略第一个字节，读取四个字节作为长度，然后解析规定长度的内容
    ///如果是java那么还要根据类型id来反序列化(可以直接由字节数组提供一个接口类型（Message）反序列化出具体类型吗？？不清楚，反正rust可以)
    // pub fn decode(bytes: Vec<u8>) -> AppResult<Message> {
    pub fn decode(src: &mut Cursor<&[u8]>) -> AppResult<Message> {
        // let content = &bytes[1..];
        let _ = src.get_u8();
        let len = src.get_i32() as usize;
        //Rust 的切片操作是左闭右开的，比如len是1那么就是第五个就够了，len是2那么就是5、6，所以右边界是len+5（不包含）
        let content = &src.get_ref()[5..5 + len];
        //为什么这里的message类型其实已经不是Message了？竟然直接是直接的枚举成员了？？？？！！
        let message = serde_json::from_slice::<Message>(content)?;
        Ok(message)
    }
}
#[test]
fn test_encode() {
    let message = Message::WorkErrorMessage();
    let string = serde_json::to_string(&message).unwrap();
    println!("{string}");
    let vec = serde_json::to_vec(&message).unwrap();
    println!("结果是{}", String::from_utf8(vec).unwrap());
    // let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    // let message1 =
    //     Message::PairRequestMessage("test".into(), DeviceInfo::new(socket, "xbss".into()));
    // let string = serde_json::to_string(&message1).unwrap();
    // println!("{string}");
    // println!("{:?}", string.as_bytes());
    // let _ = message1.encode();
}
