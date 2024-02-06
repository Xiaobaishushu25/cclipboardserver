use bytes::Buf;
use std::io::Cursor;

#[test]
fn test_cursor() {
    let bytes = "nihao 1245748".as_bytes();
    println!("bytes {:?} len {}", bytes, bytes.len());
    let mut buf = Cursor::new(bytes);
    let first = buf.get_u8();
    println!("first {:?}", first);
    let len = buf.get_ref().len();
    println!("len {:?}", len);
}
