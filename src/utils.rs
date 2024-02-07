use rand::Rng;
use rand::thread_rng;

pub fn generate_pair_code() -> String {
    let mut rng = thread_rng();
    let number: u64 = rng.gen_range(100000..1000000); // 生成100000到999999之间的随机数  
    number.to_string()
}
#[test]
fn testss() {
    let mut count = 0;
    let result = loop {
        count += 1;

        if count ==10{
            break count *10;
        }
    };
    println!("{}",result);
}