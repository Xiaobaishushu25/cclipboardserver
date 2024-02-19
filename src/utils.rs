use log::info;
use rand::thread_rng;
use rand::Rng;

pub fn generate_pair_code() -> String {
    let mut rng = thread_rng();
    let number: u64 = rng.gen_range(100000..1000000); // 生成100000到999999之间的随机数
    number.to_string()
}
///生成随机9位id
pub fn generate_id() -> String {
    // 定义可能的字符集
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    // 创建一个随机数生成器
    let mut rng = thread_rng();
    // 生成随机字符串
    let random_string: String = (0..9)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    random_string
}
#[test]
fn testss() {
    let mut count = 0;
    let result = loop {
        count += 1;

        if count == 10 {
            break count * 10;
        }
    };
    info!("{}", result);
}
