#![no_std]

mod command_parser;
mod inverse_kinematics;
mod motion;

pub use command_parser::{Command, parse_command};
pub use inverse_kinematics::solve;
pub use motion::sin_profile;
