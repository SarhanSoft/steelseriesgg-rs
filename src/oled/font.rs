//! Embedded bitmap font for the OLED screen.
//!
//! The glyphs below were drawn for this project (5x7 pixels, printable ASCII plus `°`, `…` and
//! a box for anything else). The large size is not stored: it is the small glyph doubled with
//! the Scale2x (EPX) rule, which rounds diagonals instead of producing 2x2 blocks.
//!
//! Each small glyph is seven rows, top to bottom; in each row bit 4 is the leftmost column and
//! bit 0 the rightmost.

/// Glyph size to render with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontSize {
    /// 5x7 glyphs on a 6-pixel advance and 8-pixel line: 21 characters x 5 lines on 128x40.
    #[default]
    Small,
    /// 10x14 glyphs on a 12-pixel advance and 16-pixel line: 10 characters x 2 lines on 128x40.
    Large,
}

impl FontSize {
    /// Width of one glyph's ink box in pixels.
    pub const fn glyph_width(self) -> u32 {
        match self {
            FontSize::Small => SMALL_WIDTH,
            FontSize::Large => SMALL_WIDTH * 2,
        }
    }

    /// Height of one glyph's ink box in pixels.
    pub const fn glyph_height(self) -> u32 {
        match self {
            FontSize::Small => SMALL_HEIGHT,
            FontSize::Large => SMALL_HEIGHT * 2,
        }
    }

    /// Blank columns between two glyphs.
    pub const fn spacing(self) -> u32 {
        match self {
            FontSize::Small => 1,
            FontSize::Large => 2,
        }
    }

    /// Horizontal distance from one glyph's left edge to the next.
    pub const fn advance(self) -> u32 {
        self.glyph_width() + self.spacing()
    }

    /// Vertical distance from one text line's top to the next.
    pub const fn line_height(self) -> u32 {
        self.glyph_height() + self.spacing()
    }
}

/// Small glyph width in pixels.
pub const SMALL_WIDTH: u32 = 5;
/// Small glyph height in pixels.
pub const SMALL_HEIGHT: u32 = 7;

const ROWS: usize = SMALL_HEIGHT as usize;

/// One rendered glyph: up to 14 rows of up to 16 columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    width: u32,
    height: u32,
    /// Bit `width - 1 - x` of `rows[y]` is pixel `(x, y)`.
    rows: [u16; ROWS * 2],
}

impl Glyph {
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether pixel `(x, y)` of the glyph is inked. Outside the glyph reads as blank.
    pub fn pixel(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.rows[y as usize] >> (self.width - 1 - x) & 1 == 1
    }
}

/// The glyph for `c` at `size`. Characters without a glyph render as a hollow box.
pub fn glyph(c: char, size: FontSize) -> Glyph {
    let small = small_rows(c);
    match size {
        FontSize::Small => {
            let mut rows = [0u16; ROWS * 2];
            for (dst, src) in rows.iter_mut().zip(small.iter()) {
                *dst = u16::from(*src);
            }
            Glyph {
                width: SMALL_WIDTH,
                height: SMALL_HEIGHT,
                rows,
            }
        }
        FontSize::Large => scale2x(small),
    }
}

/// Whether `c` has its own glyph (as opposed to the fallback box).
pub fn has_glyph(c: char) -> bool {
    matches!(c, ' '..='~' | '°' | '…')
}

fn small_rows(c: char) -> &'static [u8; ROWS] {
    match c {
        ' '..='~' => &ASCII[c as usize - 0x20],
        '°' => &DEGREE,
        '…' => &ELLIPSIS,
        _ => &MISSING,
    }
}

/// Double a 5x7 glyph with Scale2x: each source pixel P becomes a 2x2 block whose corners take
/// a neighbour's value when the two neighbours touching that corner agree and the opposite two
/// do not. Pixels outside the glyph count as blank.
fn scale2x(src: &[u8; ROWS]) -> Glyph {
    let w = SMALL_WIDTH as i32;
    let h = SMALL_HEIGHT as i32;
    let at = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        src[y as usize] >> (w - 1 - x) & 1 == 1
    };

    let out_w = SMALL_WIDTH * 2;
    let mut rows = [0u16; ROWS * 2];
    for y in 0..h {
        for x in 0..w {
            let p = at(x, y);
            let up = at(x, y - 1);
            let right = at(x + 1, y);
            let left = at(x - 1, y);
            let down = at(x, y + 1);

            let top_left = if left == up && left != down && up != right {
                up
            } else {
                p
            };
            let top_right = if up == right && up != left && right != down {
                right
            } else {
                p
            };
            let bottom_left = if down == left && down != right && left != up {
                left
            } else {
                p
            };
            let bottom_right = if right == down && right != up && down != left {
                down
            } else {
                p
            };

            let ox = (x * 2) as u32;
            let oy = (y * 2) as usize;
            let set = |row: &mut u16, col: u32, on: bool| {
                if on {
                    *row |= 1 << (out_w - 1 - col);
                }
            };
            set(&mut rows[oy], ox, top_left);
            set(&mut rows[oy], ox + 1, top_right);
            set(&mut rows[oy + 1], ox, bottom_left);
            set(&mut rows[oy + 1], ox + 1, bottom_right);
        }
    }

    Glyph {
        width: out_w,
        height: SMALL_HEIGHT * 2,
        rows,
    }
}

const MISSING: [u8; ROWS] = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];
const DEGREE: [u8; ROWS] = [0b01100, 0b10010, 0b10010, 0b01100, 0b00000, 0b00000, 0b00000];
const ELLIPSIS: [u8; ROWS] = [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b10101];

/// Printable ASCII, `0x20` (space) through `0x7E` (`~`).
#[rustfmt::skip]
const ASCII: [[u8; ROWS]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // ' '
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100], // '!'
    [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '"'
    [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010], // '#'
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100], // '$'
    [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011], // '%'
    [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101], // '&'
    [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '\''
    [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010], // '('
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000], // ')'
    [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000], // '*'
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000], // '+'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000], // ','
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000], // '-'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100], // '.'
    [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000], // '/'
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // '0'
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // '1'
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111], // '2'
    [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110], // '3'
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // '4'
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // '5'
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // '6'
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // '7'
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // '8'
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // '9'
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000], // ':'
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000], // ';'
    [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010], // '<'
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000], // '='
    [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000], // '>'
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100], // '?'
    [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110], // '@'
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // 'A'
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // 'B'
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110], // 'C'
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100], // 'D'
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // 'E'
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // 'F'
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111], // 'G'
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // 'H'
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 'I'
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100], // 'J'
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // 'K'
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // 'L'
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001], // 'M'
    [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001], // 'N'
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // 'O'
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // 'P'
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101], // 'Q'
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // 'R'
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110], // 'S'
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // 'T'
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // 'U'
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // 'V'
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010], // 'W'
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001], // 'X'
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100], // 'Y'
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // 'Z'
    [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110], // '['
    [0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001], // '\\'
    [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110], // ']'
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000], // '^'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111], // '_'
    [0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '`'
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111], // 'a'
    [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110], // 'b'
    [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110], // 'c'
    [0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111], // 'd'
    [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110], // 'e'
    [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000], // 'f'
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // 'g'
    [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001], // 'h'
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110], // 'i'
    [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100], // 'j'
    [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010], // 'k'
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 'l'
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001], // 'm'
    [0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001], // 'n'
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110], // 'o'
    [0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000], // 'p'
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001], // 'q'
    [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000], // 'r'
    [0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110], // 's'
    [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110], // 't'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101], // 'u'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // 'v'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010], // 'w'
    [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001], // 'x'
    [0b00000, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // 'y'
    [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111], // 'z'
    [0b00010, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00010], // '{'
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // '|'
    [0b01000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01000], // '}'
    [0b00000, 0b00000, 0b01000, 0b10101, 0b00010, 0b00000, 0b00000], // '~'
];

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii_art(g: &Glyph) -> Vec<String> {
        (0..g.height())
            .map(|y| (0..g.width()).map(|x| if g.pixel(x, y) { '#' } else { '.' }).collect())
            .collect()
    }

    #[test]
    fn every_glyph_fits_five_columns() {
        for (i, rows) in ASCII.iter().enumerate() {
            for row in rows {
                assert!(*row < 0b100000, "glyph {:#04x} has a row wider than 5 bits", i + 0x20);
            }
        }
    }

    #[test]
    fn printable_ascii_is_covered_and_distinct() {
        for c in '!'..='~' {
            assert!(has_glyph(c));
            let g = glyph(c, FontSize::Small);
            assert!((0..7).any(|y| (0..5).any(|x| g.pixel(x, y))), "{c:?} has no ink");
        }
        for (i, a) in ASCII.iter().enumerate() {
            for (j, b) in ASCII.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a,
                    b,
                    "glyphs {:?} and {:?} are identical",
                    (i + 0x20) as u8 as char,
                    (j + 0x20) as u8 as char
                );
            }
        }
    }

    #[test]
    fn small_glyph_bits_match_the_table() {
        let a = ascii_art(&glyph('A', FontSize::Small));
        assert_eq!(a, [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]);

        let one = ascii_art(&glyph('1', FontSize::Small));
        assert_eq!(one, ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]);
    }

    #[test]
    fn unknown_characters_render_as_a_box() {
        assert!(!has_glyph('é'));
        let g = ascii_art(&glyph('é', FontSize::Small));
        assert_eq!(g[0], "#####");
        assert_eq!(g[3], "#...#");
        assert_eq!(g[6], "#####");
    }

    #[test]
    fn large_glyphs_double_and_round_diagonals() {
        let g = glyph('1', FontSize::Large);
        assert_eq!((g.width(), g.height()), (10, 14));
        // A vertical stroke stays a clean 2-pixel column (rows 4-5 and 11 are rounded where
        // the flag and the base meet the stem).
        for y in 6..11 {
            assert!(g.pixel(4, y) && g.pixel(5, y), "stem row {y}");
            assert!(!g.pixel(3, y) && !g.pixel(6, y), "stem row {y} is two pixels wide");
        }

        // '/' steps from (4,0) to (3,1). Plain doubling would leave (7,1) dark; Scale2x fills
        // that inside corner so the diagonal reads as a line instead of a staircase.
        let slash = glyph('/', FontSize::Large);
        assert!(slash.pixel(8, 0) && slash.pixel(9, 1));
        assert!(slash.pixel(7, 1), "inside corner is filled");
        assert!(!slash.pixel(6, 0), "outside of the diagonal stays dark");
    }

    #[test]
    fn large_glyph_ink_covers_the_small_glyph() {
        // A lit source pixel can lose at most one corner of its 2x2 block under Scale2x,
        // because the conditions for losing two corners contradict each other.
        for c in '!'..='~' {
            let small = glyph(c, FontSize::Small);
            let large = glyph(c, FontSize::Large);
            for y in 0..7 {
                for x in 0..5 {
                    if small.pixel(x, y) {
                        let lit = [(0, 0), (1, 0), (0, 1), (1, 1)]
                            .iter()
                            .filter(|(dx, dy)| large.pixel(x * 2 + dx, y * 2 + dy))
                            .count();
                        assert!(lit >= 3, "{c:?} lost pixel ({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn metrics() {
        assert_eq!(FontSize::Small.advance(), 6);
        assert_eq!(FontSize::Small.line_height(), 8);
        assert_eq!(FontSize::Large.advance(), 12);
        assert_eq!(FontSize::Large.line_height(), 16);
    }
}
