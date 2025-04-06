pub fn console_icon_err() {
    println!("\x1b[1A\x1b[36C{}", "\x1b[1;91m✗\x1b[0m");
}

pub fn console_icon_ok() {
    println!("\x1b[1A\x1b[36C{}", "\x1b[1;92m✓\x1b[0m");
}
