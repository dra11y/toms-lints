#![allow(unused)]

fn main() {
    let name = "abc";
    let width = 10;
    //~v uninlined_format_args
    let _formatted = format!("[{:^1$}]", name, width);
}
