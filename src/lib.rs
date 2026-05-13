#![no_std]

mod command_parser;
mod inverse_kinematics;

pub use command_parser::{Command, parse_command};
pub use inverse_kinematics::solve;
