#![no_std]
#![no_main]

use atomic_float::AtomicF32;
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

const BASE_STEPS_PER_REVOLUTION: u32 = 200 * 8 * 14; // 200 steps/rev * microsteps * 14:1 gear ratio
const ARM_STEPS_PER_REVOLUTION: u32 = 200 * 8 * 5; // 200 steps/rev * microsteps * 5:1 gear ratio
/// Max = 90.1 degrees, Min = 0.0 degrees
const ARM_BOUNDS: (f32, f32) = (90.1, 0.0);
/// Max = 90.1 degrees, Min = -90.1 degrees
const BASE_BOUNDS: (f32, f32) = (90.1, -90.1);
static CURRENT_ARM_ANGLE: AtomicF32 = AtomicF32::new(0.0);
static CURRENT_BASE_ANGLE: AtomicF32 = AtomicF32::new(0.0);
static HOMING_ACTIVE: AtomicBool = AtomicBool::new(true);

bind_interrupts!(struct Irqs {
    USART2 => embassy_stm32::usart::InterruptHandler<USART2>;
    DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<DMA1_CH1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<DMA1_CH2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut _arm_enabled_pin = Output::new(p.PC12, Level::Low, Speed::Medium);
    let mut _base_enabled_pin = Output::new(p.PC6, Level::Low, Speed::Medium);

    let arm_limit_pin = Input::new(p.PC3, Pull::Up);
    let mut arm_step_pin = Output::new(p.PC11, Level::Low, Speed::Medium);
    let mut arm_dir_pin = Output::new(p.PC10, Level::Low, Speed::Medium);
    let mut base_step_pin = Output::new(p.PC9, Level::Low, Speed::Medium);
    let mut base_dir_pin = Output::new(p.PC8, Level::Low, Speed::Medium);

    let mut uart_config = Config::default();
    uart_config.baudrate = 115200;

    let mut uart = Uart::new(
        p.USART2,
        p.PA3,
        p.PA2,
        p.DMA1_CH1,
        p.DMA1_CH2,
        Irqs,
        uart_config,
    )
    .unwrap();

    homing_sequence(&arm_limit_pin, &mut arm_step_pin, &mut arm_dir_pin).await;

    // continuously read/write to UART
    loop {
        let mut buf = [0u8; 128];
        uart.read_until_idle(&mut buf).await.unwrap();

        let mut command = heapless::Vec::<u8, 128>::new();

        for &byte in &buf[..buf.len()] {
            if byte == b'\r' {
                continue;
            }

            // end received
            if byte == b'\n' {
                break;
            }

            command.push(byte).ok();
        }

        if let Ok(msg) = str::from_utf8(&command) {
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

                        if !in_arm_bounds(arm) || !in_base_bounds(base) {
                            info!("outofbounds");
                            uart.write(b"Motion Error: desired position is out of bounds\n")
                                .await
                                .unwrap();

                            continue;
                        }

                        join(
                            move_arm_to(arm, &mut arm_step_pin, &mut arm_dir_pin),
                            move_base_to(base, &mut base_step_pin, &mut base_dir_pin),
                        )
                        .await;
                    }
                    Command::RotateArm { angle } => {
                        if !in_arm_bounds(angle) {
                            uart.write(b"Motion Error: desired position is out of bounds\n")
                                .await
                                .unwrap();

                            continue;
                        }

                        move_arm_to(angle, &mut arm_step_pin, &mut arm_dir_pin).await;
                    }
                    Command::RotateBase { angle } => {
                        if !in_base_bounds(angle) {
                            uart.write(b"Motion Error: desired position is out of bounds\n")
                                .await
                                .unwrap();

                            continue;
                        }

                        move_base_to(angle, &mut base_step_pin, &mut base_dir_pin).await;
                    }
                    Command::JogArmUp => {
                        let angle = CURRENT_ARM_ANGLE.load(Ordering::Relaxed) + 1.0;

                        if !in_arm_bounds(angle) {
                            continue;
                        }

                        move_arm_to(angle, &mut arm_step_pin, &mut arm_dir_pin).await;
                    }
                    Command::JogArmDown => {
                        let angle = CURRENT_ARM_ANGLE.load(Ordering::Relaxed) - 1.0;

                        if !in_arm_bounds(angle) {
                            continue;
                        }

                        move_arm_to(angle, &mut arm_step_pin, &mut arm_dir_pin).await;
                    }
                    Command::JogBaseRight => {
                        let angle = CURRENT_BASE_ANGLE.load(Ordering::Relaxed) + 1.0;

                        if !in_base_bounds(angle) {
                            continue;
                        }

                        move_base_to(angle, &mut base_step_pin, &mut base_dir_pin).await;
                    }
                    Command::JogBaseLeft => {
                        let angle = CURRENT_BASE_ANGLE.load(Ordering::Relaxed) - 1.0;

                        if !in_base_bounds(angle) {
                            continue;
                        }

                        move_base_to(angle, &mut base_step_pin, &mut base_dir_pin).await;
                    }
                    Command::Home => {
                        HOMING_ACTIVE.store(true, Ordering::Relaxed);

                        homing_sequence(&arm_limit_pin, &mut arm_step_pin, &mut arm_dir_pin).await;
                    }
                },
                Err(e) => {
                    uart.write(e.as_bytes()).await.unwrap();
                    continue;
                }
            }
        } else {
            uart.write(b"Parse Error: invalid UTF-8\n").await.unwrap();
            continue;
        }

        uart.write(b"Command Received\n").await.unwrap();
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
    dir_pin.set_level(if reverse { Level::High } else { Level::Low });

    step_pin.set_high();
    Timer::after_micros(delay_per_step as u64).await;
    step_pin.set_low();
    Timer::after_micros(delay_per_step as u64).await;
}

/// Moves the base stepper motor to the specified angle, calculating the steps required to achieve the motion.
///
/// - `angle`: the angle (in degrees) to move the stepper too
/// - `step_pin`: the stepper driver STEP pin
/// - `dir_pin`: the stepper driver DIR pin
async fn move_base_to(angle: f32, step_pin: &mut Output<'static>, dir_pin: &mut Output<'static>) {
    let rev = BASE_STEPS_PER_REVOLUTION as f32 / 360.0;
    let num_steps = (rev * angle) as i32;
    let normalized_steps = num_steps - (rev * (CURRENT_BASE_ANGLE.load(Ordering::Relaxed))) as i32;
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

    CURRENT_BASE_ANGLE.store(angle, Ordering::Relaxed);
}

/// Moves the arm stepper motor to the specified angle, calculating the steps required to achieve the motion.
///
/// - `angle`: the angle (in degrees) to move the stepper too
/// - `step_pin`: the stepper driver STEP pin
/// - `dir_pin`: the stepper driver DIR pin
async fn move_arm_to(angle: f32, step_pin: &mut Output<'static>, dir_pin: &mut Output<'static>) {
    let rev = ARM_STEPS_PER_REVOLUTION as f32 / 360.0;
    let num_steps = (rev * angle) as i32;
    let normalized_steps = num_steps - (rev * (CURRENT_ARM_ANGLE.load(Ordering::Relaxed))) as i32;
    let increment = f32::consts::PI / normalized_steps as f32;
    let mut x = increment.clone();

    for _ in 0..normalized_steps.abs() {
        single_step(
            if normalized_steps < 0 { true } else { false },
            sin_profile(x / 10.0),
            step_pin,
            dir_pin,
        )
        .await;

        x += increment;
    }

    CURRENT_ARM_ANGLE.store(angle, Ordering::Relaxed);
}

async fn homing_sequence(
    limit_pin: &Input<'static>,
    step_pin: &mut Output<'static>,
    dir_pin: &mut Output<'static>,
) {
    HOMING_ACTIVE.store(true, Ordering::Relaxed);

    // move until limit pin is low
    while limit_pin.is_high() {
        single_step(true, 500, step_pin, dir_pin).await;
    }

    CURRENT_ARM_ANGLE.store(0.0, Ordering::Relaxed);

    move_arm_to(45.0, step_pin, dir_pin).await;

    HOMING_ACTIVE.store(false, Ordering::Relaxed);
}

fn in_arm_bounds(angle: f32) -> bool {
    (ARM_BOUNDS.1..ARM_BOUNDS.0).contains(&angle)
}
fn in_base_bounds(angle: f32) -> bool {
    (BASE_BOUNDS.1..BASE_BOUNDS.0).contains(&angle)
}
