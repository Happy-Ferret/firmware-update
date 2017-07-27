use core::ptr;
use ecflash::{Ec, EcFile, EcFlash};
use orbclient::{Color, Renderer};
use uefi::reset::ResetType;
use uefi::status::{Error, Result, Status};

use display::{Display, Output};
use exec::shell;
use fs::{find, load};
use image::{self, Image};
use io::wait_key;
use proto::Protocol;
use text::TextDisplay;

use super::ec::EcFlasher;

fn bios() -> Result<()> {
    println!("");

    find("\\system76-firmware-update\\res\\firmware.nsh")?;

    let status = shell("\\system76-firmware-update\\res\\firmware.nsh bios verify")?;
    if status != 0 {
        println!("Failed to verify BIOS: {}", status);
        return Err(Error::DeviceError);
    }

    let status = shell("\\system76-firmware-update\\res\\firmware.nsh bios flash")?;
    if status != 0 {
        println!("Failed to flash BIOS: {}", status);
        return Err(Error::DeviceError);
    }

    Ok(())
}

fn ec(master: bool) -> Result<()> {
    let (name, rom) = if master {
        ("EC", "\\system76-firmware-update\\firmware\\ec.rom")
    } else {
        ("EC2", "\\system76-firmware-update\\firmware\\ec2.rom")
    };

    println!("");

    find("\\system76-firmware-update\\res\\firmware.nsh")?;

    println!("{}: Opening device", name);
    let mut ec = match EcFlash::new(master) {
        Ok(ec) => ec,
        Err(err) => {
            println!("{}: Failed to open device: {}", name, err);
            return Err(Error::NotFound);
        }
    };
    {
        println!("{}: EC {} {} {}", name, ec.project(), ec.version(), ec.size());
    }

    println!("{}: Opening ROM file", name);
    let new_data = match load(rom) {
        Ok(new_data) => new_data,
        Err(err) => {
            println!("{}: Failed to open ROM file: {:?}", name, err);
            return Err(err);
        }
    };

    let mut new = EcFile::new(new_data.clone());
    {
        println!("{}: New {} {} {}", name, new.project(), new.version(), new.size());
    }

    if new.size() != ec.size() {
        println!("{}: New size mismatch", name);
        return Err(Error::DeviceError);
    }

    if new.project() != ec.project() {
        println!("{}: New project mismatch", name);
        return Err(Error::DeviceError);
    }

    if new.version() == ec.version() {
        println!("{}: Up to date", name);
        return Ok(());
    }

    let c = wait_key()?;

    {
        let mut flasher = match unsafe { EcFlasher::new(&mut ec) } {
            Ok(flasher) => flasher,
            Err(()) => {
                println!("{}: Failed to unlock", name);
                return Err(Error::DeviceError);
            }
        };

        println!("{}: Reading current data", name);
        let mut current_data = vec![0; flasher.size()];
        if let Err(()) = unsafe { flasher.read(&mut current_data) } {
            println!("{}: Failed to read current data", name);
            return Err(Error::DeviceError);
        }

        let mut current = EcFile::new(current_data);
        {
            println!("{}: Current {} {} {}", name, current.project(), current.version(), current.size());
        }

        if current.size() != flasher.size() || current.size() != new.size()  {
            println!("{}: Current size mismatch", name);
            return Err(Error::DeviceError);
        }

        if current.project() != flasher.project() || current.project() != new.project() {
            println!("{}: Current project mismatch", name);
            return Err(Error::DeviceError);
        }

        if current.version() != flasher.version() || current.version() == new.version()  {
            println!("{}: Current version mismatch", name);
            return Err(Error::DeviceError);
        }

        println!("{}: Erasing current data", name);
        if let Err(()) = unsafe { flasher.erase() } {
            println!("{}: Failed to erase current data", name);
            return Err(Error::DeviceError);
        }

        println!("{}: Verifying erase", name);
        let mut erase_data = vec![0; flasher.size()];
        if let Err(()) = unsafe { flasher.read(&mut erase_data) } {
            println!("{}: Failed to verify erase", name);
            return Err(Error::DeviceError);
        }

        for &b in erase_data.iter() {
            if b != 0xFF {
                println!("{}: Failed to verify erase: {:X}", name, b);
                return Err(Error::DeviceError);
            }
        }

        println!("{}: Writing new data", name);
        if let Err(()) = unsafe { flasher.write(&new_data) } {
            println!("{}: Failed to write new data", name);
            return Err(Error::DeviceError);
        }

        println!("{}: Verifying write", name);
        let mut verify_data = vec![0; flasher.size()];
        if let Err(()) = unsafe { flasher.read(&mut verify_data) } {
            println!("{}: Failed to verify write", name);
            return Err(Error::DeviceError);
        }

        let mut verify = EcFile::new(verify_data);
        {
            println!("{}: Verify {} {} {}", name, verify.project(), verify.version(), verify.size());
        }

        if verify.size() != flasher.size() || verify.size() != current.size() || verify.size() != new.size() {
            println!("{} verify size mismatch", name);
            return Err(Error::DeviceError);
        }

        if verify.project() != flasher.project() || verify.project() != current.project() || verify.project() != new.project() {
            println!("{} verify project mismatch", name);
            return Err(Error::DeviceError);
        }

        if verify.version() == flasher.version() || verify.version() == current.version() || verify.version() != new.version()  {
            println!("{} verify version mismatch", name);
            return Err(Error::DeviceError);
        }

        drop(flasher);
    }

    {
        println!("{}: EC {} {} {}", name, ec.project(), ec.version(), ec.size());
    }

    Ok(())
}

fn inner() -> Result<!> {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ValidateKind {
        Found,
        Mismatch,
        NotFound,
        Error(Error)
    }

    let validate = |name: &str, path: &str, ec_master: bool| -> ValidateKind {
        let loading = "Loading";

        print!("{}: {}", name, loading);

        let res = load(path);

        for _c in loading.chars() {
            print!("\x08");
        }

        let ret = match res {
            Ok(data) => {
                match EcFlash::new(ec_master).map(|mut ec| ec.project()) {
                    Ok(sys_project) => {
                        if EcFile::new(data).project() == sys_project {
                            ValidateKind::Found
                        } else {
                            ValidateKind::Mismatch
                        }
                    },
                    Err(_err) => {
                        ValidateKind::Mismatch
                    }
                }
            },
            Err(err) => if err == Error::NotFound {
                ValidateKind::NotFound
            } else {
                ValidateKind::Error(err)
            }
        };

        println!("{:?}", ret);

        ret
    };

    let has_bios = validate("BIOS Update", "\\system76-firmware-update\\firmware\\bios.rom", true);
    let has_ec = validate("EC Update", "\\system76-firmware-update\\firmware\\ec.rom", true);
    let has_ec2 = validate("EC2 Update", "\\system76-firmware-update\\firmware\\ec2.rom", false);

    if has_bios == ValidateKind::Found || has_ec == ValidateKind::Found {
        println!("Press enter to commence flashing");
        let c = wait_key()?;
        if c == '\n' || c == '\r' {
            if has_bios == ValidateKind::Found {
                match bios() {
                    Ok(()) => {
                        println!("Flashing BIOS: Success");
                    },
                    Err(err) => {
                        println!("Flashing BIOS: Failure: {:?}", err);
                    }
                }
            }

            if has_ec == ValidateKind::Found {
                match ec(true) {
                    Ok(()) => {
                        println!("Flashing EC: Success");
                    },
                    Err(err) => {
                        println!("Flashing EC: Failure: {:?}", err);
                    }
                }
            }

            if has_ec2 == ValidateKind::Found {
                match ec(false) {
                    Ok(()) => {
                        println!("Flashing EC2: Success");
                    },
                    Err(err) => {
                        println!("Flashing EC2: Failure: {:?}", err);
                    }
                }
            }
        }
    } else {
        println!("No updates found.");
    }

    println!("Press any key to exit");
    wait_key()?;

    unsafe {
        ((&mut *::UEFI).RuntimeServices.ResetSystem)(ResetType::Cold, Status(0), 0, ptr::null());
    }
}

pub fn main() -> Result<()> {
    let mut display = {
        let output = Output::one()?;

        let mut max_i = 0;
        let mut max_w = 0;
        let mut max_h = 0;

        for i in 0..output.0.Mode.MaxMode {
            let mut mode_ptr = ::core::ptr::null_mut();
            let mut mode_size = 0;
            (output.0.QueryMode)(output.0, i, &mut mode_size, &mut mode_ptr)?;

            let mode = unsafe { &mut *mode_ptr };
            let w = mode.HorizontalResolution;
            let h = mode.VerticalResolution;
            if w >= max_w && h >= max_h {
                max_i = i;
                max_w = w;
                max_h = h;
            }
        }

        let _ = (output.0.SetMode)(output.0, max_i);

        Display::new(output)
    };

    let mut splash = Image::new(0, 0);
    {
        println!("Loading Splash...");
        if let Ok(data) = load("\\system76-firmware-update\\res\\splash.bmp") {
            if let Ok(image) = image::bmp::parse(&data) {
                splash = image;
            }
        }
        println!(" Done");
    }

    {
        let bg = Color::rgb(0x41, 0x3e, 0x3c);

        display.set(bg);

        {
            let x = (display.width() as i32 - splash.width() as i32)/2;
            let y = 16;
            splash.draw(&mut display, x, y);
        }

        {
            let prompt = "Firmware Updater";
            let mut x = (display.width() as i32 - prompt.len() as i32 * 8)/2;
            let y = display.height() as i32 - 64;
            for c in prompt.chars() {
                display.char(x, y, c, Color::rgb(0xff, 0xff, 0xff));
                x += 8;
            }
        }

        {
            let prompt = "Do not disconnect your power adapter";
            let mut x = (display.width() as i32 - prompt.len() as i32 * 8)/2;
            let y = display.height() as i32 - 32;
            for c in prompt.chars() {
                display.char(x, y, c, Color::rgb(0xff, 0, 0));
                x += 8;
            }
        }

        display.sync();
    }

    {
        let cols = 80;
        let off_x = (display.width() as i32 - cols as i32 * 8)/2;
        let off_y = 16 + splash.height() as i32 + 16;
        let rows = (display.height() as i32 - 64 - off_y - 1) as usize/16;
        display.rect(off_x, off_y, cols as u32 * 8, rows as u32 * 16, Color::rgb(0, 0, 0));
        display.sync();

        let mut text = TextDisplay::new(&mut display);
        text.off_x = off_x;
        text.off_y = off_y;
        text.cols = cols;
        text.rows = rows;
        text.pipe(inner)?;
    }
}
