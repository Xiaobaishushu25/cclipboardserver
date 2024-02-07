use crate::app_errors::AppError::IncompleteError;
use crate::app_errors::AppResult;
use bytes::Buf;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::net::SocketAddr;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct DeviceInfo {
    // socket_addr:SocketAddr,
    device_name:String,
    device_type:DeviceType,
    // pair_code:Option<String>
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub enum DeviceType{
    DeskTop,
    Phone
}
impl DeviceInfo {
    fn new(device_name:String,) -> Self{
        Self{
            device_name,
            device_type:DeviceType::DeskTop
        }
    }
}
#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum Message {
    //六位随机数配对码
    //将自己设备信息和配对码发送给请求配对
    PairRequestMessage(String,DeviceInfo),
    //将自己设备信息发送给请求服务器创建一个配对码
    PairCreateMessage(DeviceInfo),
    PairDeviceInfosResponseMessage(Vec<DeviceInfo>),
    //用于服务器返回配对码给客户端
    PairCodeResponseMessage(String),
    //剪切板内容content
    ClipboardMessage(String),
    //服务端是否准备好了
    ServerReadyResponseMessage(),
    //没有配对的设备
    NoPairDeviceResponseMessage(),
    //请求服务端移除配对通道的指定的设备
    RemovePairRequestMessage(SocketAddr),
    //被移除配对通知
    RemovePairResponseMessage(),
    //工作失败错误，用于start_work跳出循环用
    WorkErrorMessage()
}
impl Message {
    ///检查src是否是一个完整的message，根据协议，第一个字节是消息体长度
    pub fn check_entire_message(src: &mut Cursor<&[u8]>) -> AppResult<()> {
        // println!(" self.remaining(){} src{}",src.remaining(),src.get_ref().len());
        // !src.has_remaining() || src.get_u8() == src.get_ref().len() as u8
        if !src.has_remaining() {
            Err(IncompleteError)
        } else {
            //src.get_u8()获取第一个u8，并将游标向后推一位
            let size = src.get_u8();
            let remain_size = src.get_ref().len() - 1;
            if size == (remain_size as u8) {
                Ok(())
            } else {
                Err(IncompleteError)
            }
        }
    }
    ///编码消息，转移所有权，
    /// 编码方式为：第一位放消息体的长度；剩下的放消息
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(256);
        let content = serde_json::to_vec(&self).unwrap();
        bytes.push(content.len() as u8); //第一部分放一个u8，代表消息长度
        bytes.extend(content); //第二部分代表内容
        bytes
    }

    pub fn decode(bytes: Vec<u8>) -> AppResult<Message> {
        let content = &bytes[1..];
        //为什么这里的message类型其实已经不是Message了？竟然直接是直接的枚举成员了？？？？！！
        let message = serde_json::from_slice::<Message>(content)?;
        Ok(message)
    }
}
