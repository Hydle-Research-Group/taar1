#![no_main]
#![no_std]

use core::panic::PanicInfo;
use cortex_m_rt::entry;
use hal::prelude::*;
use hal::stm32;
use rtt_target::{rprintln, rtt_init_print};
use stm32g4xx_hal as hal;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    rprintln!("{}", _info);
    loop {}
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().expect("cannot take peripherals");
    let cp = cortex_m::Peripherals::take().expect("cannot take core peripherals");

    // build the Reset & Clock Control (RCC) configuration + system timers
    let mut rcc = dp.RCC.constrain();
    let mut system_timer = cp.SYST.delay(&rcc.clocks);

    // gpio config
    let gpioa = dp.GPIOA.split(&mut rcc);
    let mut led = gpioa.pa5.into_push_pull_output();

    loop {
        system_timer.delay_ms(500);
        led.set_low().expect("set low gone wrong");
        system_timer.delay_ms(500);
        led.set_high().expect("set high gone wrong");

        rprintln!("LED is HIGH? {}", led.is_set_high().unwrap());
    }
}
