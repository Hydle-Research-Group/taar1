pub enum Command {
    MoveTo { x: f32, y: f32, z: f32 },
}

fn parse_to_array(command: &str) -> [&str; 10] {
    let mut parts = [""; 10]; // 10 = max command allocation

    for (i, part) in command.split(',').enumerate() {
        if i < parts.len() {
            parts[i] = part;
        }
    }

    parts
}

pub fn parse_command(command: &str) -> Result<Command, &str> {
    let parts = parse_to_array(command);

    if parts[0] == "moveto" {
        if parts[4..10] != [""; 6] {
            return Err(
                "Syntax Error: invalid inputs on 'moveto'\nUsage: moveto [x: float] [y: float] [z: float]",
            );
        }

        let x = parts[1]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [x], expected type float")?;
        let y = parts[2]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [y], expected type float")?;
        let z = parts[3]
            .parse::<f32>()
            .map_err(|_| "Syntax Error: invalid argument to [z], expected type float")?;

        return Ok(Command::MoveTo { x, y, z });
    }

    Err("Invalid Command: command is unknown")
}
