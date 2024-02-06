use crate::app_errors::AppError::IncompleteError;
use crate::app_errors::AppResult;
use bytes::Buf;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    //username、pwd、email、code
    //username、pwd
    PairRequestMessage(String),
    //content
    ClipboardMessage(String),
    //服务端是否准备好了
    ServerReadyMessage(),
}
impl Message {
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

    pub fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(256);
        let content = serde_json::to_vec(&self).unwrap();
        // bytes.push(message.get_type_id());//第一部分放一个u8，代表类型
        bytes.push(content.len() as u8); //第一部分放一个u8，代表消息长度
        bytes.extend(content); //第二部分代表内容
        bytes
    }

    // pub fn decode_message(bytes:Vec<u8>) ->Message{
    pub fn decode(bytes: Vec<u8>) -> AppResult<Message> {
        // let type_id = bytes.get(0).unwrap();
        // let struct_name = TYPE_TO_STRUCT.get().unwrap().get(type_id).unwrap();
        let content = &bytes[1..];
        //为什么这里的message类型其实已经不是Message了？竟然直接是直接的枚举成员了？？？？！！
        let message = serde_json::from_slice::<Message>(content)?;
        // serde_json::from_slice::<Message>(content)
        Ok(message)
    }
}
// pub struct Serializer;
// impl Serializer{
//     pub fn encode_message(message: Message)->Vec<u8> {
//         let mut bytes = Vec::with_capacity(256);
//         let content = serde_json::to_vec(&message).unwrap();
//         // bytes.push(message.get_type_id());//第一部分放一个u8，代表类型
//         bytes.push(content.len() as u8);//第一部分放一个u8，代表消息长度
//         bytes.extend(content);//第二部分代表内容
//         bytes
//     }
//     pub fn decode_message(bytes:Vec<u8>) ->Message{
//         let type_id = bytes.get(0).unwrap();
//         // let struct_name = TYPE_TO_STRUCT.get().unwrap().get(type_id).unwrap();
//         let content = &bytes[1..];
//         //为什么这里的message类型其实已经不是Message了？竟然直接是直接的枚举成员了？？？？！！
//         let message = serde_json::from_slice::<Message>(content).unwrap();
//         message
//     }
// }
