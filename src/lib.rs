#![no_std]

mod command_parser;

pub use command_parser::{Command, parse_command};
