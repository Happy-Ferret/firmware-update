// This is really, really dangerous to run from userspace right now!
// The procedure needs to be improved!
// Lockups have happened requiring complete battery burnout to fix!

use alloc::string::String;
use ecflash::{Ec, EcFlash};
use uefi::status::{Error, Result as UefiResult};

pub struct EcFlasher<'a> {
    ec: &'a mut EcFlash,
    size: usize,
    project: String,
    version: String,
    a: u8,
    b: u8,
}

impl<'a> Ec for EcFlasher<'a> {
    fn size(&mut self) -> usize {
        self.size
    }

    fn project(&mut self) -> String {
        self.project.clone()
    }

    fn version(&mut self) -> String {
        self.version.clone()
    }
}

impl<'a> EcFlasher<'a> {
    pub unsafe fn new(ec: &'a mut EcFlash) -> Result<EcFlasher, ()> {
        let project = ec.project();
        let version = ec.version();
        let size = ec.size();

        ec.set_param(0xf9, 0x20)?;
        ec.set_param(0xfa, 0x02)?;
        ec.set_param(0xfb, 0x00)?;
        ec.set_param(0xf8, 0xb1)?;
        let (a, b) = match ec.get_param(0xf9)? & 0xf0 {
            0x40 => (0xc0, 0x03),
            0x80 => (0xff, 0x04),
            _ => (0x80, 0x01)
        };

        ec.cmd(0xde)?;
        ec.cmd(0xdc)?;
        ec.cmd(0xf0)?;
        ec.read()?;

        Ok(EcFlasher {
            ec: ec,
            size: size,
            project: project,
            version: version,
            a: a,
            b: b,
        })
    }

    pub unsafe fn read(&mut self, data: &mut [u8]) -> Result<(), ()> {
        println!("Read {} KB", self.size/1024);

        let mut bytes = data.iter_mut();

        let ec = &mut self.ec;
        for i in 0..self.size/65536 {
            ec.cmd(0x03)?;
            ec.cmd(i as u8)?;

            print!("Block {}: ", i);

            for _j in 0..64 {
                for _k in 0..1024 {
                    if let Some(b) = bytes.next() {
                        *b = ec.read()?;
                    } else {
                        ec.read()?;
                    }
                }
                print!("*");
            }

            println!("");
        }

        Ok(())
    }

    pub unsafe fn erase(&mut self) -> Result<(), ()> {
        println!("Erase {} KB", self.size/1024);

        let uefi = &mut *::UEFI;

        let ec = &mut self.ec;
        ec.cmd(0x01)?;
        ec.cmd(0x00)?;
        ec.cmd(0x00)?;
        ec.cmd(0x00)?;
        ec.cmd(0x00)?;

        print!("Erasing: ");

        for _i in 0..64 {
            let _ = (uefi.BootServices.Stall)(15000);
            print!("*");
        }

        println!("");

        Ok(())
    }

    pub unsafe fn write(&mut self, data: &[u8]) -> Result<(), ()> {
        println!("Write {} KB", self.size/1024);

        let mut bytes = data.iter();

        let ec = &mut self.ec;
        for i in 0..self.size/65536 {
            ec.cmd(0x02)?;
            ec.cmd(0x00)?;
            ec.cmd(i as u8)?;
            /*
            if i == 0 {
                ec.cmd(0x04)?;
            } else {
                ec.cmd(0x00)?;
            }
            */
            ec.cmd(0x00)?;
            ec.cmd(0x00)?;

            print!("Block {}: ", i);

            for _j in 0..64 {
                for _k in 0..1024 {
                    if let Some(b) = bytes.next() {
                        ec.write(*b)?;
                    } else {
                        ec.write(0xFF)?;
                    }
                }
                print!("*");
            }

            println!("");
        }

        Ok(())
    }
}

pub fn main() -> UefiResult<()> {
    let mut ec = match EcFlash::new(true) {
        Ok(ec) => ec,
        Err(err) => {
            println!("EC open error: {}", err);
            return Err(Error::DeviceError);
        }
    };

    println!("EC: {} {} {}", ec.project(), ec.version(), ec.size());

    Ok(())
}
