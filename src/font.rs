use crate::config::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Px {
    Blank,
    On,
    Off,
    Colon,
}

const ASCII: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

// Segments a..g as bits 0..6, and the 3x5 pixels each one covers.
const DIGIT_SEGMENTS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
const SEGMENT_PIXELS: [[(usize, usize); 3]; 7] = [
    [(0, 0), (1, 0), (2, 0)],
    [(2, 0), (2, 1), (2, 2)],
    [(2, 2), (2, 3), (2, 4)],
    [(0, 4), (1, 4), (2, 4)],
    [(0, 2), (0, 3), (0, 4)],
    [(0, 0), (0, 1), (0, 2)],
    [(0, 2), (1, 2), (2, 2)],
];

fn glyph(view: View, ch: char) -> Vec<Vec<Px>> {
    let d = ch.to_digit(10).map(|d| d as usize);
    match (view, d) {
        (View::Binary, Some(d)) => [8, 4, 2, 1].iter().map(|b| vec![if d & b != 0 { Px::On } else { Px::Off }]).collect(),
        (View::Binary, None) => vec![vec![Px::Blank]; 4],
        (_, None) => (0..5).map(|r| vec![if r == 1 || r == 3 { Px::Colon } else { Px::Blank }]).collect(),
        (View::Digital, Some(d)) => {
            let mut g = vec![vec![Px::Blank; 3]; 5];
            for (seg, pixels) in SEGMENT_PIXELS.iter().enumerate() {
                let lit = DIGIT_SEGMENTS[d] & (1 << seg) != 0;
                for &(x, y) in pixels {
                    if lit {
                        g[y][x] = Px::On;
                    } else if g[y][x] == Px::Blank {
                        g[y][x] = Px::Off;
                    }
                }
            }
            g
        }
        (_, Some(d)) => ASCII[d]
            .iter()
            .map(|row| (0..3).map(|x| if row & (4 >> x) != 0 { Px::On } else { Px::Off }).collect())
            .collect(),
    }
}

/// Pixel rows for a time string like `12:34:56`, glyphs separated by one blank column.
pub fn bitmap(view: View, text: &str) -> Vec<Vec<Px>> {
    let glyphs: Vec<_> = text.chars().map(|c| glyph(view, c)).collect();
    let rows = glyphs.first().map_or(0, Vec::len);
    (0..rows)
        .map(|r| {
            let mut row = Vec::new();
            for (i, g) in glyphs.iter().enumerate() {
                if i > 0 {
                    row.push(Px::Blank);
                }
                row.extend(&g[r]);
            }
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(rows: &[Vec<Px>]) -> Vec<String> {
        let c = |p: &Px| match p {
            Px::On => '#',
            Px::Off => '.',
            Px::Colon => ':',
            Px::Blank => ' ',
        };
        rows.iter().map(|r| r.iter().map(c).collect()).collect()
    }

    #[test]
    fn ascii_digits() {
        assert_eq!(show(&bitmap(View::Ascii, "4:2")), ["#.#   ###", "#.# : ..#", "###   ###", "..# : #..", "..#   ###"]);
    }

    #[test]
    fn digital_shows_unlit_segments() {
        assert_eq!(show(&bitmap(View::Digital, "1")), ["..#", ". #", "..#", ". #", "..#"]);
        assert_eq!(show(&bitmap(View::Digital, "8")), ["###", "# #", "###", "# #", "###"]);
    }

    #[test]
    fn binary_bits() {
        assert_eq!(show(&bitmap(View::Binary, "09:5")), [". #   .", ". .   #", ". .   .", ". #   #"]);
    }
}
