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

//! I2S character device wrapper.
//!
//! Bridges the `blueos_hal::i2s::I2s` HAL trait to the kernel `Device` trait,
//! exposing `/dev/i2s0` for userspace write (playback) and read (capture).

use crate::devices::{Device, DeviceClass, DeviceId, DeviceManager};
use alloc::{string::String, sync::Arc};
use blueos_hal::{
    i2s::{I2s, I2sConfig},
    Configuration, PlatPeri,
};
use embedded_io::ErrorKind;

pub struct I2sDevice<D: 'static> {
    driver: &'static D,
    configured: core::sync::atomic::AtomicBool,
    codec_status_reported: core::sync::atomic::AtomicBool,
}

impl<D> I2sDevice<D>
where
    D: I2s<I2sConfig, ()> + PlatPeri + Configuration<I2sConfig, Target = ()> + Send + Sync,
{
    pub fn new(driver: &'static D) -> Self {
        Self {
            driver,
            configured: core::sync::atomic::AtomicBool::new(false),
            codec_status_reported: core::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn register(self, name: &str) -> Result<(), ErrorKind> {
        let device = Arc::new(self);
        DeviceManager::get().register_device(String::from(name), device)
    }

    fn ensure_configured(&self) -> Result<(), ErrorKind> {
        let configured = self.configured.load(core::sync::atomic::Ordering::Relaxed);
        if configured {
            return Ok(());
        }
        log::info!("[I2S_DEVICE] calling driver.enable()");
        self.driver.enable();
        log::info!("[I2S_DEVICE] driver.enable() complete; configuring 16kHz");
        self.driver
            .configure(&I2sConfig::default_16k())
            .map_err(|_| ErrorKind::InvalidData)?;
        self.configured
            .store(true, core::sync::atomic::Ordering::Relaxed);
        log::info!("[I2S_DEVICE] configure complete; configured=true");
        Ok(())
    }

    fn report_codec_status(&self) {
        if !self
            .codec_status_reported
            .swap(true, core::sync::atomic::Ordering::Relaxed)
        {
            match crate::drivers::audio::speaker_power_status() {
                1 => log::error!("[AUDIO] speaker power=failed"),
                2 => log::info!("[AUDIO] speaker power=enabled"),
                _ => log::error!("[AUDIO] speaker power=not-attempted"),
            }
            match crate::drivers::audio::es8311_status() {
                1 => log::error!("[AUDIO] ES8311 init=failed"),
                2 => log::error!("[AUDIO] ES8311 init=ok verify=failed"),
                3 => log::info!("[AUDIO] ES8311 init=ok verify=ok"),
                4 => log::error!("[AUDIO] ES8311 init=skipped i2c=unavailable"),
                _ => log::error!("[AUDIO] ES8311 init=not-attempted"),
            }
        }
    }
}

impl<D> Device for I2sDevice<D>
where
    D: I2s<I2sConfig, ()> + PlatPeri + Configuration<I2sConfig, Target = ()> + Send + Sync,
{
    fn name(&self) -> String {
        String::from("i2s0")
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(1, 10)
    }

    fn open(&self) -> Result<(), ErrorKind> {
        self.ensure_configured()
    }

    fn read(&self, _pos: u64, buf: &mut [u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        self.ensure_configured()?;
        self.driver
            .read(buf)
            .map(|()| buf.len())
            .map_err(|_| ErrorKind::Other)
    }

    fn write(&self, _pos: u64, buf: &[u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        self.report_codec_status();
        self.ensure_configured()?;
        self.driver
            .write(buf)
            .map(|()| buf.len())
            .map_err(|_| ErrorKind::Other)
    }
}
