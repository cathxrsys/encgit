use colored::*;

fn lighter(r: u8, g: u8, b: u8, intensity: u8) -> (u8, u8, u8) {
    (
        r.saturating_add(intensity),
        g.saturating_add(intensity),
        b.saturating_add(intensity),
    )
}

pub fn print_banner() {
    let art = include_str!("../resources/tag.dat");

    let lines: Vec<&str> = art.lines().collect();

    for line in lines {
        print_gradient_line(line);
        println!();
    }
}

pub fn print_mini_banner() {
    let art = include_str!("../resources/minitag.dat");

    let lines: Vec<&str> = art.lines().collect();

    for line in lines {
        print_gradient_line(line);
        println!();
    }
}

fn print_gradient_line(line: &str) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len().max(1);

    for (i, c) in chars.iter().enumerate() {
        let t = i as f32 / len as f32;

        let r = (255.0 * (1.0 - t)) as u8;
        let g = (80.0 * t) as u8;
        let b = 255;

        let (r_lighter, g_lighter, b_lighter) = lighter(r, g, b, 100);

        let mut styled = c.to_string().truecolor(r, g, b);
        let styled_lighter = c.to_string().truecolor(r_lighter, g_lighter, b_lighter);

        if *c == '/' || *c == '|' || *c == '_' || *c == '\\' {
            styled = styled_lighter;
        }

        print!("{}", styled);
    }
}
