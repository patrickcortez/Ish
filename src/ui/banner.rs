use std::env;

pub fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    
    let user = env::var("USER").or_else(|_| env::var("USERNAME")).unwrap_or_else(|_| "user".to_string());
    let host = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME")).unwrap_or_else(|_| "host".to_string());

    let c1 = "\x1b[38;2;100;200;255m";
    let c2 = "\x1b[38;2;100;225;220m";
    let c3 = "\x1b[38;2;100;255;180m";
    let c4 = "\x1b[38;2;120;255;150m";
    let c5 = "\x1b[38;2;150;255;120m";
    let reset = "\x1b[0m";
    let border = "\x1b[38;5;60m";
    let text_dim = "\x1b[38;5;245m";

    println!();
    println!("{border}╭────────────────────────────────────────────────────────────╮{reset}");
    println!("{border}│{reset}                                                            {border}│{reset}");
    println!("{border}│{reset}  {c1}██████ ██████ ██  ██{reset}                                      {border}│{reset}");
    println!("{border}│{reset}  {c2}  ██   ██     ██  ██{reset}                                      {border}│{reset}");
    println!("{border}│{reset}  {c3}  ██   ██████ ██████{reset}                                      {border}│{reset}");
    println!("{border}│{reset}  {c4}  ██       ██ ██  ██{reset}                                      {border}│{reset}");
    println!("{border}│{reset}  {c5}██████ ██████ ██  ██{reset}                                      {border}│{reset}");
    println!("{border}│{reset}                                                            {border}│{reset}");
    println!("{border}├────────────────────────────────────────────────────────────┤{reset}");
    
    let mut sys_str = format!("v{}  |  {} Windows  |  {}@{}", version, "\u{e70f}", user, host);
    if os == "macos" {
        sys_str = format!("v{}  |  {} macOS  |  {}@{}", version, "\u{f179}", user, host);
    } else if os == "linux" {
        sys_str = format!("v{}  |  {} Linux  |  {}@{}", version, "\u{f17c}", user, host);
    }
    
    while sys_str.chars().count() < 58 {
        sys_str.push(' ');
    }
    if sys_str.chars().count() > 58 {
        sys_str = sys_str.chars().take(58).collect();
    }
    
    println!("{border}│{reset}  \x1b[36m{}{border}│{reset}", sys_str);
    println!("{border}│{reset}  {text_dim}Type \x1b[36mhelp{text_dim} for commands, \x1b[36mexit{text_dim} to quit.{reset}                     {border}│{reset}");
    println!("{border}│{reset}  {text_dim}Type \x1b[36mish-about{text_dim} to learn about Ishshell{reset}                    {border}│{reset}");
    println!("{border}╰────────────────────────────────────────────────────────────╯{reset}");
    println!();
}
