use uefi::status::Result;

use io::wait_key;

pub mod boot;
pub mod config;
pub mod dmi;
pub mod ec;
pub mod flash;
pub mod mouse;
pub mod vars;

pub fn menu() -> Result<()> {
    type CommandFunc = fn() -> Result<()>;
    type Command = (&'static str, CommandFunc);

    macro_rules! cmd {
        ($name:ident) => {
            (stringify!($name), self::$name::main as CommandFunc)
        };
    }

    let commands: [Command; 7] = [
        cmd!(flash),
        cmd!(boot),
        cmd!(config),
        cmd!(dmi),
        cmd!(ec),
        cmd!(mouse),
        cmd!(vars)
    ];

    loop {
        print!("0 => exit");
        for (i, cmd) in commands.iter().enumerate() {
            print!(", {} => {}", i + 1, cmd.0);
        }
        println!("");


        let c = wait_key().unwrap_or('?');

        let num = if c >= '0' && c <= '9' {
            (c as u8 - b'0') as usize
        } else {
            commands.len()
        };

        if num == 0 {
            return Ok(());
        } else if let Some(command) = commands.get(num - 1) {
            if let Err(err) = command.1() {
                println!("Failed to run {}: {:?}", command.0, err);
            }
        } else {
            println!("Invalid selection '{}'", c);
        }
    }
}
