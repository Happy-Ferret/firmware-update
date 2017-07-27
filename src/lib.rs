#![no_std]
#![feature(alloc)]
#![feature(asm)]
#![feature(compiler_builtins_lib)]
#![feature(concat_idents)]
#![feature(const_fn)]
#![feature(global_allocator)]
#![feature(lang_items)]
#![feature(never_type)]
#![feature(try_trait)]

#[macro_use]
extern crate alloc;
extern crate compiler_builtins;
extern crate dmi;
extern crate ecflash;
extern crate orbclient;
extern crate plain;
extern crate uefi;
extern crate uefi_alloc;

use core::ptr;
use text::pipe;

#[global_allocator]
static ALLOCATOR: uefi_alloc::Allocator = uefi_alloc::Allocator;

pub static mut HANDLE: uefi::Handle = uefi::Handle(0);
pub static mut UEFI: *mut uefi::system::SystemTable = 0 as *mut uefi::system::SystemTable;

#[macro_use]
mod macros;

pub mod cmd;
pub mod display;
pub mod exec;
pub mod externs;
pub mod fs;
pub mod image;
pub mod io;
pub mod loaded_image;
pub mod panic;
pub mod pointer;
pub mod proto;
pub mod rt;
pub mod shell;
pub mod string;
pub mod text;

fn main() {
    let uefi = unsafe { &mut *::UEFI };

    let _ = (uefi.BootServices.SetWatchdogTimer)(0, 0, 0, ptr::null());

    let _ = (uefi.ConsoleOut.SetAttribute)(uefi.ConsoleOut, 0x0F);

    /*
    if let Err(err) = cmd::flash::main() {
        println!("Flashing error: {:?}", err);
    }
    */

    if let Err(err) = pipe(cmd::menu) {
        println!("{:?}", err);
    }
}
