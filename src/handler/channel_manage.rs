use crate::message::message::{DeviceInfo, Message};
use crate::utils::generate_pair_code;
use log::info;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

pub struct ChannelManage {
    stream_channel: HashMap<SocketAddr, Sender<Message>>,
    //key值是PairCode，  value是一个元组，
    // 第一个元素是sender（泛型元组.0是发送的信息，元组.1是addr，用于判断送消息是否来自自己）
    // 第二个元素是当前配对的设备信息列表
    // pair_channel:HashMap<String,(Sender<(String, SocketAddr)>, Vec<DeviceInfo>)>
    pair_channel: HashMap<String, (Sender<(Message, SocketAddr)>, Vec<DeviceInfo>)>,
}
impl ChannelManage {
    pub fn new() -> Self {
        Self {
            stream_channel: HashMap::new(),
            pair_channel: HashMap::new(),
        }
    }
    pub fn get_socket_num(&mut self) -> usize {
        self.stream_channel.len()
    }
    pub fn get_pair_num(&mut self) -> usize {
        self.pair_channel.len()
    }
    pub fn add_stream_channel(&mut self, addr: SocketAddr, tx: Sender<Message>) {
        self.stream_channel.insert(addr, tx);
    }
    ///根据SocketAddr拿到管道发送端
    pub fn get_stream_channel(&self, addr: SocketAddr) -> Option<Sender<Message>> {
        self.stream_channel.get(&addr).cloned()
    }
    pub fn remove_stream_channel(&mut self, addr: SocketAddr) {
        self.stream_channel.remove(&addr);
    }
    ///根据发来的配对码检查是否有配对通道，若存在即配对成功，将设备信息插到设备群中并返回该通道的发送句柄和设备群信息，不存在返回None
    // pub fn get_pair_channel(&mut self,pair_code:&str,device_info: &DeviceInfo)-> Option<(Sender<(String, SocketAddr)>,Vec<DeviceInfo>)> {
    pub fn get_pair_channel(
        &mut self,
        pair_code: &str,
        device_info: &DeviceInfo,
        can_create:bool
    ) -> Option<(Sender<(Message, SocketAddr)>, Vec<DeviceInfo>)> {
        match self.pair_channel.get_mut(pair_code) {
            // None => None,
            None => {
                if can_create {
                    //没有配对的，创建一个，设备信息也只有自己
                    let (rx,_) = self.add_pair_channel(Some(pair_code.to_string()), device_info.clone());
                    Some((rx,vec![device_info.clone()]))
                }else {
                    None
                }
            },
            Some(tuple) => {
                let x = &mut (tuple.1);
                x.push(device_info.clone());
                // Some(((&tuple.0).clone(), x.clone()))
                Some((tuple.0.clone(), x.clone()))
            }
        }
    }
    ///根据配对码添加一个配对通道，并返回此通道发送句柄和配对码
    // pub fn add_pair_channel(&mut self,device_info: DeviceInfo) ->(Sender<(String, SocketAddr)>,String) {
    pub fn add_pair_channel(
        &mut self,
        pair_code: Option<String>,
        device_info: DeviceInfo,
    ) -> (Sender<(Message, SocketAddr)>, String) {
        let (tx, _) = broadcast::channel(20);
        let r_tx = tx.clone(); //return的tx的意思
        let code = match pair_code {
            None => {
                loop {
                    let code = generate_pair_code();
                    if self.pair_channel.get(&code).is_none() {
                        self.pair_channel
                            .insert(code.clone(), (tx, vec![device_info]));
                        break code;
                    };
                }
            }
            Some(code) => {
                self.pair_channel
                    .insert(code.clone(), (tx, vec![device_info]));
                code
            }
        };
        // let code = loop {
        //     // let code = generate_pair_code();
        //     // if self.pair_channel.get(&code).is_none() {
        //     //     self.pair_channel
        //     //         .insert(code.clone(), (tx, vec![device_info]));
        //     //     break code;
        //     // };
        // };
        (r_tx, code)
    }
    ///根据配对码，检查配对设备列表，如果存在给定的设备，将其移除。若设备表只有一个，直接删除键值对。
    pub fn remove_pair_device(&mut self, pair_code: &str, device_info: DeviceInfo) {
        if let Some(tuple) = self.pair_channel.get_mut(pair_code) {
            let device_list = &mut tuple.1;
            device_list.retain(|device| device != &device_info);
            if device_list.is_empty() {
                info!("{} all disconnected", pair_code);
                self.pair_channel.remove(pair_code);
            }
            // 本来是下面这样写的，先判断是不是1效率应该更高点，但是我发现本地测试时两个一样的DeviceInfo
            //直接清没了，不走len==1了，所以还是改了吧。虽然应该不会有一样的设备。
            // if device_list.len()==1{
            //     info!("{}全部断开连接",pair_code);
            //     self.pair_channel.remove(pair_code);
            // }else {
            //     device_list.retain(|device| device!=&device_info);
            // }
        }
    }
}
