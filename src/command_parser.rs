pub enum Command {
    MoveTo { x: f32, y: f32, z: f32 },
    Home,
}

fn parse_to_array(command: &str) -> ([&str; 10], usize) {
    let mut parts = [""; 10];
    let mut count = 0;

    for (i, part) in command.split_whitespace().enumerate() {
        if i < parts.len() {
            parts[i] = part;
            count = i + 1;
        }
    }

    (parts, count)
}

/// Parses a `str` into a `Command` enum.
///
/// - `command`: the `str` object to parse
///
/// # Errors
///
/// This function will error if the `command` cannot be parsed into a `Command` enum (due to invalid arguments)
///
/// Additionally, this function will error if the command is invalid or unknown.
pub fn parse_command(command: &str) -> Result<Command, &str> {
    let (parts, len) = parse_to_array(command);
    let cmd = parts[0];

    if cmd == "moveto" {
        if len != 4 {
            return Err("Usage: moveto [x: float] [y: float] [z: float]\n");
        }

        let x = parts[1]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [x], expected type float\n")?;
        let y = parts[2]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [y], expected type float\n")?;
        let z = parts[3]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [z], expected type float\n")?;

        return Ok(Command::MoveTo { x, y, z });
    } else if cmd == "home" {
        if parts[1..10] != [""; 9] {
            return Err("Syntax Error: invalid arguments to 'home' function; Usage: home\n");
        }

        return Ok(Command::Home);
    }

    Err("Invalid Command: command is unknown\n")
}
