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

//! Raw device for a reserved ESP32-C6 internal Flash region.

use super::internal_flash::{with_internal_flash, EspFlashError, ESP_FLASH_SECTOR_SIZE};
use crate::devices::{Device, DeviceClass, DeviceId, DeviceManager};
use alloc::{string::String, sync::Arc};
use embedded_io::ErrorKind;

pub const ESP32_FLASH_DEVICE_NAME: &str = "esp32-flash0";
pub const ESP32_FLASH_ERASE_RANGE: u32 = 0x40;
pub const FLASH_IOCTL_ABI_VERSION: u32 = 1;

const REGION_BASE: u32 = blueos_kconfig::CONFIG_ESP32_FLASH_IO_REGION_BASE as u32;
const REGION_SIZE: u32 = blueos_kconfig::CONFIG_ESP32_FLASH_IO_REGION_SIZE as u32;

#[repr(C)]
struct EraseRangeRequest {
    version: u32,
    size: u32,
    flags: u32,
    region_offset: u32,
    length: u32,
}

#[derive(Debug, Clone, Copy)]
struct InternalFlashRegion {
    base: u32,
    size: u32,
}

impl InternalFlashRegion {
    const fn new(base: u32, size: u32) -> Self {
        Self { base, size }
    }

    fn absolute_offset(&self, offset: u32, len: usize) -> Result<u32, EspFlashError> {
        let len = u32::try_from(len).map_err(|_| EspFlashError::OutOfBounds)?;
        let end = offset.checked_add(len).ok_or(EspFlashError::OutOfBounds)?;
        if end > self.size {
            return Err(EspFlashError::OutOfBounds);
        }
        self.base
            .checked_add(offset)
            .ok_or(EspFlashError::OutOfBounds)
    }

    fn validate(&self) -> Result<(), EspFlashError> {
        if self.base % ESP_FLASH_SECTOR_SIZE as u32 != 0
            || self.size == 0
            || self.size % ESP_FLASH_SECTOR_SIZE as u32 != 0
        {
            return Err(EspFlashError::UnalignedErase);
        }
        self.base
            .checked_add(self.size)
            .ok_or(EspFlashError::OutOfBounds)?;
        Ok(())
    }
}

struct Esp32FlashDevice {
    region: InternalFlashRegion,
}

impl Esp32FlashDevice {
    const fn new(region: InternalFlashRegion) -> Self {
        Self { region }
    }

    fn erase(&self, arg: usize) -> Result<(), ErrorKind> {
        if arg == 0 || arg % core::mem::align_of::<EraseRangeRequest>() != 0 {
            return Err(ErrorKind::InvalidInput);
        }
        let request = unsafe { core::ptr::read_volatile(arg as *const EraseRangeRequest) };
        if request.version != FLASH_IOCTL_ABI_VERSION
            || request.size != core::mem::size_of::<EraseRangeRequest>() as u32
            || request.flags != 0
            || request.length == 0
            || request.region_offset % ESP_FLASH_SECTOR_SIZE as u32 != 0
            || request.length % ESP_FLASH_SECTOR_SIZE as u32 != 0
        {
            return Err(ErrorKind::InvalidInput);
        }
        let physical = self
            .region
            .absolute_offset(request.region_offset, request.length as usize)
            .map_err(map_error)?;
        with_internal_flash(|flash| flash.erase_region(physical, request.length)).map_err(map_error)
    }
}

impl Device for Esp32FlashDevice {
    fn name(&self) -> String {
        String::from(ESP32_FLASH_DEVICE_NAME)
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Misc
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(1, 0x35)
    }

    fn read(&self, pos: u64, buf: &mut [u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        if pos >= self.region.size as u64 {
            return Ok(0);
        }
        let count = core::cmp::min(buf.len() as u64, self.region.size as u64 - pos) as usize;
        let relative = u32::try_from(pos).map_err(|_| ErrorKind::InvalidInput)?;
        let physical = self
            .region
            .absolute_offset(relative, count)
            .map_err(map_error)?;
        with_internal_flash(|flash| flash.read(physical, &mut buf[..count])).map_err(map_error)?;
        Ok(count)
    }

    fn write(&self, pos: u64, buf: &[u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        let relative = u32::try_from(pos).map_err(|_| ErrorKind::InvalidInput)?;
        let physical = self
            .region
            .absolute_offset(relative, buf.len())
            .map_err(map_error)?;
        with_internal_flash(|flash| flash.write(physical, buf)).map_err(map_error)?;
        Ok(buf.len())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        match request {
            ESP32_FLASH_ERASE_RANGE => self.erase(arg),
            _ => Err(ErrorKind::Unsupported),
        }
    }

    fn capacity(&self) -> Result<u64, ErrorKind> {
        Ok(self.region.size as u64)
    }

    fn sector_size(&self) -> Result<u16, ErrorKind> {
        Ok(ESP_FLASH_SECTOR_SIZE as u16)
    }

    fn sync(&self) -> Result<(), ErrorKind> {
        Ok(())
    }
}

fn map_error(error: EspFlashError) -> ErrorKind {
    match error {
        EspFlashError::OutOfBounds | EspFlashError::UnalignedErase => ErrorKind::InvalidInput,
        EspFlashError::Busy | EspFlashError::RomError(_) => ErrorKind::Other,
    }
}

pub fn init_esp32_flash_device() -> Result<(), ErrorKind> {
    let region = InternalFlashRegion::new(REGION_BASE, REGION_SIZE);
    region.validate().map_err(map_error)?;
    DeviceManager::get().register_device(
        String::from(ESP32_FLASH_DEVICE_NAME),
        Arc::new(Esp32FlashDevice::new(region)),
    )?;
    crate::kprintln!(
        "[FLASH] registered /dev/{} region=0x{:08x}..0x{:08x}",
        ESP32_FLASH_DEVICE_NAME,
        REGION_BASE,
        REGION_BASE + REGION_SIZE
    );
    Ok(())
}
