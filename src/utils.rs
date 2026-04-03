use std::io;
use std::io::BufRead;

pub fn wait_until_enter() {
    println!("按回车键继续...");
    io::stdin().lock().read_line(&mut String::new()).unwrap();
}
