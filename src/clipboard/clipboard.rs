use arboard::Clipboard;
use clipboard_master::{CallbackResult, ClipboardHandler};
use std::io;

struct ClipboardListen {
    clipboard: Clipboard,
}
impl ClipboardListen {
    fn new() -> Self {
        Self {
            clipboard: Clipboard::new().unwrap(),
        }
    }
}
impl ClipboardHandler for ClipboardListen {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        println!("Clipboard text was: {}", self.clipboard.get_text().unwrap());
        // println!("Clipboard change happened!");
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
        eprintln!("Error: {}", error);
        CallbackResult::Next
    }
}
enum TransferDataType {
    Text,
    Image,
    Html,
}
struct TransferData {
    data_type: TransferDataType,
    data_content: Vec<u8>,
}
