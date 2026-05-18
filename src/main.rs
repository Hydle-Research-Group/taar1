#![no_std]
#![no_main]

use core::f32;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::gpio::{AnyPin, Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{DMA1_CH1, DMA1_CH2, USART2};
use embassy_stm32::usart::{Config, Uart};
use embassy_stm32::{Peri, bind_interrupts};
use embassy_time::Timer;
use taar1::{Command, parse_command, sin_profile, solve};
use {defmt_rtt as _, panic_probe as _};

enum StepperType {
    Base,
    Arm,
}

const BASE_STEPS_PER_REVOLUTION: u32 = 14 * 2720;
const ARM_STEPS_PER_REVOLUTION: u32 = 1000;
const MAX_BASE_ROTATION: f32 = 180.0;
const MAX_ARM_ROTATION: f32 = 90.0;
static CURRENT_BASE_STEPS: AtomicI32 = AtomicI32::new(0);
static CURRENT_ARM_STEPS: AtomicI32 = AtomicI32::new(0);
static HOMING_ACTIVE: AtomicBool = AtomicBool::new(true);

bind_interrupts!(struct Irqs {
    USART2 => embassy_stm32::usart::InterruptHandler<USART2>;
    DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<DMA1_CH1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<DMA1_CH2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let mut uart = Uart::new(
        p.USART2,
        p.PA3,
        p.PA2,
        p.DMA1_CH1,
        p.DMA1_CH2,
        Irqs,
        Config::default(),
    )
    .unwrap();

    // spawner.spawn(homing_sequence(/* p.<PIN>.into() */).unwrap());

    // continuously read/write to UART
    loop {
        let mut buffer = [0_u8; 256]; // 64 char limit
        uart.read(&mut buffer).await.unwrap();

        let msg = str::from_utf8(&buffer).unwrap();

        match parse_command(msg) {
            Ok(cmd) => match cmd {
                Command::MoveTo { x, y, z } => {
                    if HOMING_ACTIVE.load(Ordering::Relaxed) {
                        uart.write(b"Motion Error: machine is actively homing\n")
                            .await
                            .unwrap();

                        continue;
                    }

                    let (base, arm) = solve(x, y, z);

                    if (base > MAX_BASE_ROTATION || arm > MAX_ARM_ROTATION)
                        || (base < 0.0 || arm < 0.0)
                    {
                        uart.write(
                            b"Motion Error: desired position is beyond the machine limits\n",
                        )
                        .await
                        .unwrap();

                        continue;
                    }

                    // join(
                    // move_stepper_to(StepperType::Base, base, step_pin, dir_pin),
                    // move_stepper_to(StepperType::Arm, arm, step_pin, dir_pin),
                    // ).await;
                }
                Command::Home => {
                    HOMING_ACTIVE.store(true, Ordering::Relaxed);
                }
            },
            Err(e) => uart.write(e.as_bytes()).await.unwrap(),
        }
    }
}

/// Sends a single step pulse to the specified step pin.
///
/// - `reverse`: if the step should be reversed (sets `dir_pin` HIGH if `true`)
/// - `delay_per_step`: how much delay between the pulse
/// - `step_pin`: the stepper driver STEP pin
/// - `dir_pin`: the stepper driver DIR pin
async fn single_step(
    reverse: bool,
    delay_per_step: u32,
    step_pin: &mut Output<'static>,
    dir_pin: &mut Output<'static>,
) {
    if reverse {
        dir_pin.set_high();
    }

    step_pin.set_high();
    Timer::after_micros(delay_per_step as u64).await;
    step_pin.set_low();
    Timer::after_micros(delay_per_step as u64).await;
}

/// Moves the stepper motor to the specified angle, calculating the steps required to achieve the motion.
///
/// - `stepper`: the stepper type to load for delta calculation
/// - `angle`: the angle (in degrees) to move the stepper too
/// - `step_pin`: the stepper driver STEP pin
/// - `dir_pin`: the stepper driver DIR pin
async fn move_stepper_to(
    stepper: StepperType,
    angle: f32,
    step_pin: &mut Output<'static>,
    dir_pin: &mut Output<'static>,
) {
    let num_steps = ((if matches!(stepper, StepperType::Base) {
        BASE_STEPS_PER_REVOLUTION
    } else {
        ARM_STEPS_PER_REVOLUTION
    } as f32
        / 360.0)
        * angle) as i32;
    let normalized_steps = num_steps
        - if matches!(stepper, StepperType::Base) {
            CURRENT_BASE_STEPS.load(Ordering::Relaxed)
        } else {
            CURRENT_ARM_STEPS.load(Ordering::Relaxed)
        };
    let increment = f32::consts::PI / normalized_steps as f32;
    let mut x = increment.clone();

    for _ in 0..normalized_steps.abs() {
        single_step(
            if normalized_steps < 0 { true } else { false },
            sin_profile(x),
            step_pin,
            dir_pin,
        )
        .await;

        x += increment;
    }

    if matches!(stepper, StepperType::Base) {
        CURRENT_BASE_STEPS.store(num_steps, Ordering::Relaxed)
    } else {
        CURRENT_ARM_STEPS.store(num_steps, Ordering::Relaxed);
    }
}

#[embassy_executor::task]
async fn homing_sequence(
    base_limit_pin: Peri<'static, AnyPin>,
    arm_limit_pin: Peri<'static, AnyPin>,
    base_step_pin: Peri<'static, AnyPin>,
    arm_step_pin: Peri<'static, AnyPin>,
    base_dir_pin: Peri<'static, AnyPin>,
    arm_dir_pin: Peri<'static, AnyPin>,
) {
    let base_limit_pin = Input::new(base_limit_pin, Pull::Up);
    let arm_limit_pin = Input::new(arm_limit_pin, Pull::Up);
    let mut base_step_pin = Output::new(base_step_pin, Level::Low, Speed::Low);
    let mut arm_step_pin = Output::new(arm_step_pin, Level::Low, Speed::Low);
    let mut base_dir_pin = Output::new(base_dir_pin, Level::High, Speed::Low);
    let mut arm_dir_pin = Output::new(arm_dir_pin, Level::High, Speed::Low);

    if HOMING_ACTIVE.load(Ordering::Relaxed) {
        while base_limit_pin.is_high() {
            single_step(false, 5, &mut base_step_pin, &mut base_dir_pin).await;
        }

        CURRENT_BASE_STEPS.store(0, Ordering::Relaxed);

        while arm_limit_pin.is_high() {
            single_step(false, 5, &mut arm_step_pin, &mut arm_dir_pin).await;
        }

        CURRENT_ARM_STEPS.store(0, Ordering::Relaxed);

        HOMING_ACTIVE.store(false, Ordering::Relaxed);
    }
}
