// Copyright (c) 2026 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Board key input device.
//!
//! Aggregates the two board keys into `/dev/keys`, following the NuttX
//! `/dev/buttons` pattern: a single read returns a one-byte level bitmap
//! (bit0 = Key2 held, bit1 = Key3 held). Semantic mapping lives in the
//! consumer.
//!
//! Key2 (GPIO9) and Key3 (GPIO10) are active-low inputs. The board pin-state
//! table enables their input paths and pull-ups.

use blueos_driver::i2c::I2cConfig;
use embedded_io::ErrorKind;

use crate::{
    devices::{
        bus::Bus,
        i2c_core::block_i2c::BlockI2c,
        Device, DeviceClass, DeviceData, DeviceId, DeviceManager,
    },
    drivers::{DriverModule, InitDriver, Result as DriverResult},
};
use alloc::{string::String, sync::Arc};

const KEYS_DEVICE_NAME: &str = "keys";
const KEYS_DEVICE_MAJOR: usize = 242;
const KEYS_DEVICE_MINOR: usize = 2;

/// `/dev/keys` event bitmap (1 byte).
///
/// - bit0: Key2 (GPIO9) currently held.
/// - bit1: Key3 (GPIO10) currently held.
pub const KEYS_REPORT_SIZE: usize = 1;

pub const KEY2_HELD: u8 = 1 << 0;
pub const KEY3_HELD: u8 = 1 << 1;

struct KeysState {
    last_report: Option<u8>,
}

pub struct KeysDevice {
    state: crate::sync::SpinLock<KeysState>,
}

impl KeysDevice {
    fn new() -> Self {
        Self {
            state: crate::sync::SpinLock::new(KeysState { last_report: None }),
        }
    }

    /// Collect the current key event bitmap.
    fn read_keys(&self) -> u8 {
        // ESP32-C6 GPIO_IN_REG. Both keys are active-low.
        let gpio_in = unsafe { core::ptr::read_volatile((0x6009_1000 + 0x3c) as *const u32) };
        let key2_held = gpio_in & (1 << KEY2_GPIO) == 0;
        let key3_held = gpio_in & (1 << KEY3_GPIO) == 0;
        let report = (if key2_held { KEY2_HELD } else { 0 })
            | (if key3_held { KEY3_HELD } else { 0 });

        let mut state = self.state.lock();
        if state.last_report != Some(report) {
            crate::kearly_println!(
                "[KEYS] GPIO9={} GPIO10={} key2={} key3={}",
                (!key2_held) as u8,
                (!key3_held) as u8,
                key2_held as u8,
                key3_held as u8
            );
            state.last_report = Some(report);
        }
        report
    }
}

/// GPIO input data register bits. Pressed = tied to ground.
/// The register address matches blueos_driver::gpio::esp32c6_gpio's
/// GPIO_BASE.input (GPIO + 0x3c).
const KEY2_GPIO: u32 = 9;
const KEY3_GPIO: u32 = 10;

impl Device for KeysDevice {
    fn name(&self) -> String {
        String::from(KEYS_DEVICE_NAME)
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(KEYS_DEVICE_MAJOR, KEYS_DEVICE_MINOR)
    }

    fn read(&self, _pos: u64, buf: &mut [u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        if buf.len() < KEYS_REPORT_SIZE {
            return Err(ErrorKind::InvalidInput);
        }
        buf[0] = self.read_keys();
        Ok(KEYS_REPORT_SIZE)
    }

    fn write(&self, _pos: u64, _buf: &[u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        Err(ErrorKind::Unsupported)
    }
}

// ---------------------------------------------------------------------------
// Bus-attached driver configuration (matches the battery driver pattern)
// ---------------------------------------------------------------------------

pub struct KeysConfig {}

pub struct KeysDriverModule {
    _marker: core::marker::PhantomData<()>,
}

impl KeysDriverModule {
    pub const fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()> + 'static> DriverModule<BlockI2c<T>>
    for KeysDriverModule
{
    type Data = KeysConfig;

    fn probe(_dev: &DeviceData) -> DriverResult<Self::Data> {
        Ok(KeysConfig {})
    }
}

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()> + 'static> InitDriver<BlockI2c<T>>
    for KeysConfig
{
    type Data = ();

    fn init(self, _bus: &Bus<BlockI2c<T>>) -> DriverResult<Self::Data> {
        let device = Arc::new(KeysDevice::new());
        DeviceManager::get()
            .register_device(String::from(KEYS_DEVICE_NAME), device)
            .map_err(|_| crate::error::code::EIO)?;

        crate::kearly_println!("keys registered as /dev/keys");
        log::info!("keys registered as /dev/keys");
        Ok(())
    }
}
