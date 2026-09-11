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

//! AXP2101 speaker-amplifier power control.

use blueos_driver::i2c::I2cConfig;
use embedded_hal::i2c::I2c as HalI2c;

use crate::devices::{
    bus::{Bus, BusWrapper},
    i2c_core::block_i2c::BlockI2c,
};

const AXP2101_I2C_ADDR: u8 = 0x34;
const REG_LDO_ONOFF: u8 = 0x90;
const REG_ALDO2_VOLTAGE: u8 = 0x93;
const ALDO2_ENABLE: u8 = 1 << 1;
const ALDO2_3V3: u8 = 0x1c;

fn read_reg<T>(bus: &mut BusWrapper<BlockI2c<T>>, reg: u8) -> Result<u8, crate::error::Error>
where
    T: blueos_hal::i2c::I2c<I2cConfig, ()> + 'static,
{
    let mut value = [0u8; 1];
    bus.transaction(
        AXP2101_I2C_ADDR,
        &mut [
            embedded_hal::i2c::Operation::Write(&[reg]),
            embedded_hal::i2c::Operation::Read(&mut value),
        ],
    )?;
    Ok(value[0])
}

fn write_reg<T>(
    bus: &mut BusWrapper<BlockI2c<T>>,
    reg: u8,
    value: u8,
) -> Result<(), crate::error::Error>
where
    T: blueos_hal::i2c::I2c<I2cConfig, ()> + 'static,
{
    bus.transaction(
        AXP2101_I2C_ADDR,
        &mut [embedded_hal::i2c::Operation::Write(&[reg, value])],
    )
}

/// Supply the board's speaker amplifier from AXP2101 ALDO2 at 3.3 V.
pub(crate) fn enable_speaker_power<T>(bus: &Bus<BlockI2c<T>>) -> Result<(), crate::error::Error>
where
    T: blueos_hal::i2c::I2c<I2cConfig, ()> + 'static,
{
    let mut i2c = bus.intf.clone();

    let voltage_before = read_reg(&mut i2c, REG_ALDO2_VOLTAGE)?;
    let voltage = (voltage_before & !0x1f) | ALDO2_3V3;
    if voltage != voltage_before {
        write_reg(&mut i2c, REG_ALDO2_VOLTAGE, voltage)?;
    }

    let enables_before = read_reg(&mut i2c, REG_LDO_ONOFF)?;
    let enables = enables_before | ALDO2_ENABLE;
    if enables != enables_before {
        write_reg(&mut i2c, REG_LDO_ONOFF, enables)?;
    }

    let voltage_after = read_reg(&mut i2c, REG_ALDO2_VOLTAGE)?;
    let enables_after = read_reg(&mut i2c, REG_LDO_ONOFF)?;
    if voltage_after & 0x1f != ALDO2_3V3 || enables_after & ALDO2_ENABLE == 0 {
        return Err(crate::error::code::EIO);
    }

    log::info!(
        "[AUDIO] AXP2101 ALDO2 power=enabled voltage_reg=0x{:02x} ldo_reg=0x{:02x}",
        voltage_after,
        enables_after
    );
    Ok(())
}
