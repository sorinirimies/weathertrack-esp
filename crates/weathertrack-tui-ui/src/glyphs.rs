//! Big digits and condition art, drawn with block characters.

/// 3×5 pixel font for the characters of a temperature.
fn glyph(c: char) -> [&'static str; 5] {
    match c {
        '0' => ["###", "# #", "# #", "# #", "###"],
        '1' => [" # ", "## ", " # ", " # ", "###"],
        '2' => ["###", "  #", "###", "#  ", "###"],
        '3' => ["###", "  #", "###", "  #", "###"],
        '4' => ["# #", "# #", "###", "  #", "  #"],
        '5' => ["###", "#  ", "###", "  #", "###"],
        '6' => ["###", "#  ", "###", "# #", "###"],
        '7' => ["###", "  #", "  #", "  #", "  #"],
        '8' => ["###", "# #", "###", "# #", "###"],
        '9' => ["###", "# #", "###", "  #", "###"],
        '-' => ["   ", "   ", "###", "   ", "   "],
        _ => ["   ", "   ", "   ", "   ", "   "],
    }
}

/// `text` as big block letters: `scale` columns per pixel horizontally, `scale / 2`-ish rows
/// vertically is not used, one row per pixel row times `scale_y`. Returns one string per row.
pub fn big(text: &str, scale_x: usize, scale_y: usize) -> Vec<String> {
    let mut rows = vec![String::new(); 5 * scale_y];
    for (i, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        for (r, line) in g.iter().enumerate() {
            let mut cells = String::new();
            for p in line.chars() {
                let c = if p == '#' { '█' } else { ' ' };
                for _ in 0..scale_x {
                    cells.push(c);
                }
            }
            if i + 1 < text.chars().count() {
                for _ in 0..scale_x.div_ceil(2).max(1) {
                    cells.push(' ');
                }
            }
            for k in 0..scale_y {
                rows[r * scale_y + k].push_str(&cells);
            }
        }
    }
    rows
}

/// Width in cells of [`big`] output.
pub fn big_width(text: &str, scale_x: usize) -> usize {
    let n = text.chars().count();
    n * 3 * scale_x + n.saturating_sub(1) * scale_x.div_ceil(2).max(1)
}

/// Five-line, nine-column art for a condition tag ("SUN", "CLD", "RAN", …).
pub fn art(tag: &str, is_day: bool) -> [&'static str; 5] {
    match tag {
        "SUN" if is_day => [
            "  \\ | /  ",
            "   .-.   ",
            "-- ( ) --",
            "   `-'   ",
            "  / | \\  ",
        ],
        "SUN" => [
            "    _.   ",
            "  .'  `. ",
            " /   *   ",
            " \\      / ",
            "  `-..-' ",
        ],
        "CLD" => [
            "         ",
            "   .--.  ",
            " .-(    ).",
            "(___.__)_)",
            "         ",
        ],
        "RAN" => [
            "   .--.  ",
            " .-(    ).",
            "(___.__)_)",
            " ' ' ' ' '",
            "' ' ' ' ' ",
        ],
        "DRZ" => [
            "   .--.  ",
            " .-(    ).",
            "(___.__)_)",
            "  ' '  ' ",
            " '  ' '  ",
        ],
        "STM" => [
            "   .--.  ",
            " .-(    ).",
            "(___.__)_)",
            "  /_/_/  ",
            "   /_/   ",
        ],
        "SNW" => [
            "   .--.  ",
            " .-(    ).",
            "(___.__)_)",
            " * * * * *",
            "* * * * * ",
        ],
        "FOG" => [
            "         ",
            " _ _ _ _ ",
            "_ _ _ _ _",
            " _ _ _ _ ",
            "         ",
        ],
        _ => [
            "         ",
            "    ?    ",
            "   ???   ",
            "    ?    ",
            "         ",
        ],
    }
}

/// A horizontal bar of `width` cells filled to `frac` (0..=1): "████░░░░".
pub fn bar(frac: f32, width: usize) -> (String, String) {
    let filled = ((frac.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    ("█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_digits_have_five_rows_and_the_right_width() {
        let rows = big("12", 2, 1);
        assert_eq!(rows.len(), 5);
        assert!(rows.iter().all(|r| r.chars().count() == big_width("12", 2)));
        assert_eq!(big("7", 1, 2).len(), 10);
    }

    #[test]
    fn minus_sign_is_a_bar() {
        let rows = big("-", 1, 1);
        assert_eq!(rows[2], "███");
        assert_eq!(rows[0], "   ");
    }

    #[test]
    fn every_art_is_five_lines_of_a_constant_width() {
        for tag in ["SUN", "CLD", "RAN", "DRZ", "STM", "SNW", "FOG", "???"] {
            for day in [true, false] {
                let a = art(tag, day);
                assert_eq!(a.len(), 5);
                let w = a[0].chars().count();
                assert!(
                    a.iter().all(|l| l.chars().count() >= 8),
                    "{tag} lines are at least 8 wide (w={w})"
                );
            }
        }
    }

    #[test]
    fn bars_fill_proportionally() {
        assert_eq!(bar(0.5, 10), ("█████".to_string(), "░░░░░".to_string()));
        assert_eq!(bar(2.0, 4).0, "████");
        assert_eq!(bar(-1.0, 4).1, "░░░░");
    }
}
