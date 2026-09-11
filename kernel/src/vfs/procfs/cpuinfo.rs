// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
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

use crate::{arch, error::Error, vfs::procfs::ProcFileOps};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;

const CPU_FREQ_KHZ_FALLBACK: u32 = 160_000;

pub(crate) struct CpuInfo;

impl ProcFileOps for CpuInfo {
    fn get_content(&self) -> Result<Vec<u8>, Error> {
        let hart = arch::current_cpu_id();
        let khz = current_cpu_frequency_khz();
        let mut result = String::with_capacity(96);
        writeln!(result, "processor       : {}", hart).unwrap();
        writeln!(result, "hart            : {}", hart).unwrap();
        writeln!(result, "isa             : rv32imac").unwrap();
        writeln!(result, "uarch           : esp32c6").unwrap();
        writeln!(
            result,
            "cpu MHz         : {}.{:03}",
            khz / 1_000,
            khz % 1_000
        )
        .unwrap();
        Ok(result.as_bytes().to_vec())
    }

    fn set_content(&self, _content: Vec<u8>) -> Result<usize, Error> {
        Ok(0)
    }
}

#[cfg(target_board = "esp32c6_devkitc_1")]
fn current_cpu_frequency_khz() -> u32 {
    const PCR_SYSCLK_CONF: *const u32 = 0x6009_6110 as *const u32;
    const PCR_CPU_FREQ_CONF: *const u32 = 0x6009_6118 as *const u32;
    const PLL_FREQ_KHZ: u32 = 480_000;
    const RC_FAST_FREQ_KHZ: u32 = 17_500;

    let sysclk = unsafe { core::ptr::read_volatile(PCR_SYSCLK_CONF) };
    let cpu = unsafe { core::ptr::read_volatile(PCR_CPU_FREQ_CONF) };
    let source = (sysclk >> 16) & 0x3;

    match source {
        0 => {
            let xtal_khz = ((sysclk >> 24) & 0x7f) * 1_000;
            let root_div = (sysclk & 0xff) + 1;
            let cpu_div = (cpu & 0xff) + 1;
            xtal_khz / root_div / cpu_div
        }
        1 => {
            let cpu_div = ((cpu >> 8) & 0xff) + 1;
            if cpu_div == 1 && cpu & (1 << 16) != 0 {
                120_000
            } else {
                let root_div = ((sysclk >> 8) & 0xff) + 1;
                PLL_FREQ_KHZ / root_div / cpu_div
            }
        }
        2 => {
            let root_div = (sysclk & 0xff) + 1;
            let cpu_div = (cpu & 0xff) + 1;
            RC_FAST_FREQ_KHZ / root_div / cpu_div
        }
        _ => CPU_FREQ_KHZ_FALLBACK,
    }
}

#[cfg(not(target_board = "esp32c6_devkitc_1"))]
fn current_cpu_frequency_khz() -> u32 {
    CPU_FREQ_KHZ_FALLBACK
}
