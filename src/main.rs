#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::Peri;
use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_time::Timer;
use steperb::StepperController;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut base = StepperController::new(200);
    base.set_desired_angle(45.0);

    let p = embassy_stm32::init(Default::default());

    spawner.spawn(step_task(base, p.PA0.into(), p.PA1.into()).unwrap());
    spawner.spawn(blinky(p.PA5.into()).unwrap());
}

#[embassy_executor::task]
async fn step_task(
    mut controller: StepperController,
    step_pin: Peri<'static, AnyPin>,
    dir_pin: Peri<'static, AnyPin>,
) {
    let mut step_pin_output = Output::new(step_pin, Level::High, Speed::Low);
    let mut dir_pin_output = Output::new(dir_pin, Level::High, Speed::Low);

    loop {
        if controller.needs_movement() {
            if controller.is_reversed() {
                dir_pin_output.set_high(); // or low
            }

            step_pin_output.set_high();
            Timer::after_micros(5).await;
            step_pin_output.set_low();

            controller.apply_step();

            Timer::after_micros(500).await;
        } else {
            Timer::after_millis(1).await;
        }
    }
}

#[embassy_executor::task]
async fn blinky(p: Peri<'static, AnyPin>) {
    let mut led = Output::new(p, Level::High, Speed::Low);

    loop {
        info!("high");
        led.set_high();
        Timer::after_millis(300).await;

        info!("low");
        led.set_low();
        Timer::after_millis(300).await;
    }
}
