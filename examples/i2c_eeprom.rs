//! Reading and page writing to an external EEPROM Microchip 24LC64 using I2C1

// #![deny(warnings)]
#![feature(proc_macro)]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_debug;
extern crate cortex_m_rtfm as rtfm;
#[macro_use]
extern crate f4;
extern crate stm32f40x;

use core::mem::transmute;
use f4::I2c;
use f4::led::{self, LED};
use rtfm::{app, Threshold};
use core::result::Result;
use stm32f40x::I2C1;
use f4::clock;

const EEPROM_PAGE_SIZE: usize = 32;
const RX_BUFFER_SIZE: usize = core::mem::size_of::<u32>();

/// Errors for reading EEPROM
#[derive(Debug)]
pub enum Error {
    /// Invalid eeprom memory address
    InvalidMemory,
}

app! {
    device: f4::stm32f40x,

    idle: {
        resources: [I2C1, ITM],
    },
}

fn init(p: init::Peripherals) {
    clock::set_84_mhz(&p.RCC, &p.FLASH);

    // Use the LED as I/O indicator
    led::init(p.GPIOA, p.RCC);

    // Init the I2C peripheral
    let i2c = I2c(p.I2C1);
    i2c.init(p.GPIOA, p.GPIOB, p.RCC);
    i2c.enable();
}

fn read_eeprom(
    i2c: &I2c<I2C1>,
    mem_addr: u16,
    rx_buffer: &mut [u8; RX_BUFFER_SIZE],
) -> Result<(), Error> {
    // Check if we are addressing inside eeprom memory space
    if mem_addr > 0x1fff - RX_BUFFER_SIZE as u16 {
        return Err(Error::InvalidMemory);
    }
    // Write device address and memory address to set eeprom internal cursor
    while i2c.start(0xa0).is_err() {}
    while i2c.write((mem_addr >> 8) as u8).is_err() {}
    while i2c.write(mem_addr as u8).is_err() {}

    // Read incoming bytes and ACK them
    while i2c.start(0xa1).is_err() {}
    for i in 0..RX_BUFFER_SIZE {
        rx_buffer[i] = loop {
            if i == RX_BUFFER_SIZE - 1 {
                // Do not ACK the last byte received and send STOP
                if let Ok(byte) = i2c.read_nack() {
                    break byte;
                }
            } else {
                // ACK the byte after receiving
                if let Ok(byte) = i2c.read_ack() {
                    break byte;
                }
            }
        }
    }
    Ok(())
}

fn write_eeprom_page(
    i2c: &I2c<I2C1>,
    mem_addr: u16,
    tx_buffer: &[u8; EEPROM_PAGE_SIZE],
) -> Result<(), Error> {
    // Check if we are addressing inside eeprom memory space and address is page aligned
    if mem_addr > 0x1fff - EEPROM_PAGE_SIZE as u16 || mem_addr % EEPROM_PAGE_SIZE as u16 != 0 {
        return Err(Error::InvalidMemory);
    }
    // Write device address and memory address to set eeprom internal cursor
    while i2c.start(0xa0).is_err() {}
    while i2c.write((mem_addr >> 8) as u8).is_err() {}
    while i2c.write(mem_addr as u8).is_err() {}

    // Write data
    for i in 0..EEPROM_PAGE_SIZE {
        while i2c.write(tx_buffer[i]).is_err() {}
    }
    while i2c.stop().is_err() {}
    Ok(())
}

fn idle(_t: &mut Threshold, r: idle::Resources) -> ! {
    let i2c = I2c(r.I2C1);

    // Write in 32 byte pages (max for this eeprom)
    let mut mem_addr = 0x0000;
    let mut page: [u8; EEPROM_PAGE_SIZE] = [0; EEPROM_PAGE_SIZE];
    let mut page_index = 0;
    for (_data_index, data) in DATA.iter().enumerate() {
        // Store u32 into u8 page buffer
        let data_bytes: [u8; 4] = unsafe { transmute(data.to_le()) };
        page[page_index..(page_index + 4)].clone_from_slice(&data_bytes);
        page_index += 4;
        if page_index >= page.len() {
            page_index = 0;
            // We have filled the page, now write it.
            LED.on();
            write_eeprom_page(&i2c, mem_addr, &page).unwrap();
            LED.off();
            mem_addr += EEPROM_PAGE_SIZE as u16;
        }
    }
    // The data might not be 32 byte page aligned, so
    if DATA.len() * 4 % EEPROM_PAGE_SIZE != 0 {
        // Just send the whole page...
        write_eeprom_page(&i2c, mem_addr, &page).unwrap();
    }

    // Read back to check that it worked
    let mut rx: [u8; RX_BUFFER_SIZE] = [0; RX_BUFFER_SIZE];
    let mut status_ok = true;
    for (data_index, written_data) in DATA.iter().enumerate() {
        let mem_addr: u16 = data_index as u16 * 4;
        LED.on();
        match read_eeprom(&i2c, mem_addr, &mut rx) {
            Err(_) => {}
            Ok(_) => {
                LED.off();
                // Read the byte array as
                let read_data: u32 = unsafe { core::ptr::read(rx.as_ptr() as *const _) };
                let _w = *written_data;
                if read_data != *written_data {
                    status_ok = false;
                    break;
                }
            }
        }
    }
    if status_ok {
        LED.on();
    } else {
        LED.off();
    }
    loop {
        rtfm::bkpt()
    }
}

// The 24LC64 has place room for 64*1000/32 integers
const DATA: [u32; 2000] = [
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x243762ff, 0x59c8f80c, 0xb6e0db4b, 0xedb8cab7, 0xcd4b39f6, 0xaf263741, 0x18d9965f, 0x1ab1f037,
    0x5b458792, 0xc94d960d, 0xd45cedea, 0x2160aca3, 0x93c77766, 0x2d66e105, 0x9ff74d4f, 0x6dc22f21,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x015e7a47, 0x2ef84ebb, 0x177a8db4, 0x1b722ff9, 0x5dc7cff0, 0x5dc9dea6, 0x1da0c15a, 0xe4c236a2,
    0x3d16b0d0, 0x1f397842, 0xaae0d2ba, 0x11246674, 0x0845317f, 0xd5512dad, 0xb6184977, 0xd293a53e,
    0x7d9c2716, 0xd917eae6, 0xd8852384, 0x286e46f9, 0xce566029, 0xcefe7daf, 0x62d726d4, 0x0dbaeb2d,
    0x95f57c60, 0xed515141, 0x29b77d0f, 0x9f7b8d0c, 0x45a8395a, 0xfead2b72, 0x883d434c, 0xed8ddf60,
    0xe51e65e4, 0x19bf6bb1, 0xfeb505ec, 0x662aa23c, 0xf6827cf8, 0xd1dc7a5c, 0x4fa5b066, 0x7ddd25a4,
    0xa8ba8e8a, 0x72846227, 0xf8f636fb, 0x2b389a9c, 0xe4038bf6, 0x6e169877, 0xad028132, 0x84dbfe8c,
    0x6b03d689, 0x5fc48de0, 0x1138f000, 0xccb58e57, 0xf9c8e200, 0x7ab26e3c, 0xc61dcb3e, 0x6aefccb0,
    0x7a452f05, 0xa5cf0731, 0xa249383f, 0x628fe534, 0xcad81710, 0x7f616276, 0x3ce18308, 0xed4857ff,
    0xd1e5b1d1, 0xc2e84dc2, 0xaa003742, 0xaf637488, 0x831afc48, 0x287a69a0, 0x6e04546e, 0x13dffa07,
    0x3232fb10, 0xd69e2e09, 0x355d8dc7, 0xef902301, 0x9a89ac15, 0x967dc900, 0x08dc2b1c, 0x6b5be690,
    0x894b0e02, 0xe26af9af, 0xa6fd3b23, 0xfcf213e5, 0x85217608, 0x7fd3be8b, 0xa2e757fb, 0x3717a341,
    0x85ee426d, 0x394bb856, 0x12ac98c3, 0xec7d4ab5, 0x721b6989, 0x30e36360, 0xaa018403, 0x9ee61196,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
    0xa8697adc, 0x51e9d65a, 0x11023594, 0xc4c4b36b, 0xda80bf7a, 0xbd5a645e, 0x18cea918, 0xa723dda8,
    0x0126c05e, 0x2962d48a, 0xd5f7d312, 0xb8947041, 0x7c1e2e9a, 0x945eeac3, 0x7110fb1c, 0xa7bc72cc,
];
