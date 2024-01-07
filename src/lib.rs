//! Filter IRC colored stdin to ANSI colored stdout.
#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    clippy::cargo,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::pedantic,
    clippy::str_to_string,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    rustdoc::missing_crate_level_docs,
    rustdoc::unescaped_backticks
)]
#![allow(clippy::match_same_arms, clippy::single_match_else)]

use std::io;
use std::io::BufRead;
use std::io::Result;
use std::io::Write;

use crate::filter::BufFilter;
use crate::filter::Filter;

mod filter;

/// Stream bytes from `reader` to `writer` while translating IRC color codes into ANSI ones.
///
/// On success returns the number of bytes written to `writer`.
///
/// # Errors
///
/// This function will return an error if any call to [`read`] or [`write`] returns an error.
///
/// [`read`]: std::io::Read::read
/// [`write`]: Write::write
///
/// # Panics
///
/// This function will panic if an unknown IRC color is encountered.
///
/// # Examples
///
/// ```
/// # use std::io::BufReader;
/// #
/// # use ircat::ircat;
/// #
/// let mut writer = Vec::new();
/// ircat(&mut BufReader::new(b"Colors \x034red \x033green \x032blue\n".as_ref()), &mut writer)?;
/// assert_eq!(writer, b"Colors \x1b[31mred \x1b[32mgreen \x1b[34mblue\x1b[39m\x1b[49m\n");
/// # std::io::Result::Ok(())
/// ```
pub fn ircat<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<u64> {
    io::copy(&mut BufFilter::<IRCatFilter, R>::new(reader), writer)
}

enum IRCatState {
    Normal,
    Start,
    Foreground1(u8),
    Foreground2,
    Comma,
    Background1(u8),
}

struct IRCatFilter {
    state: IRCatState,
    in_color: bool,
}

impl Filter for IRCatFilter {
    fn init() -> Self {
        Self {
            state: IRCatState::Normal,
            in_color: false,
        }
    }

    fn filter(&mut self, input: &[u8], output: &mut Vec<u8>) {
        output.reserve(input.len());
        for c in input {
            match self.state {
                IRCatState::Normal => match c {
                    b'\x03' => {
                        self.state = IRCatState::Start;
                    }
                    b'\n' => {
                        if self.in_color {
                            output.extend_from_slice(b"\x1b[39m\x1b[49m");
                            self.in_color = false;
                        }
                        output.push(*c);
                    }
                    _ => output.push(*c),
                },

                IRCatState::Start => match c {
                    b'0'..=b'9' => {
                        self.in_color = true;
                        self.state = IRCatState::Foreground1(c - b'0');
                    }
                    _ => {
                        if self.in_color {
                            self.in_color = false;
                            output.extend_from_slice(b"\x1b[39m\x1b[49m");
                        }
                        output.push(*c);
                        self.state = IRCatState::Normal;
                    }
                },

                IRCatState::Foreground1(mut fg_color) => match c {
                    b'0'..=b'9' => {
                        fg_color = fg_color * 10 + c - b'0';
                        output_color(output, true, fg_color);
                        self.state = IRCatState::Foreground2;
                    }
                    b',' => {
                        output_color(output, true, fg_color);
                        self.state = IRCatState::Comma;
                    }
                    _ => {
                        output_color(output, true, fg_color);
                        output.push(*c);
                        self.state = IRCatState::Normal;
                    }
                },

                IRCatState::Foreground2 => match c {
                    b',' => self.state = IRCatState::Comma,
                    _ => {
                        output.push(*c);
                        self.state = IRCatState::Normal;
                    }
                },

                IRCatState::Comma => match c {
                    b'0'..=b'9' => self.state = IRCatState::Background1(c - b'0'),
                    _ => {
                        output.push(b',');
                        output.push(*c);
                        self.state = IRCatState::Normal;
                    }
                },

                IRCatState::Background1(mut bg_color) => match c {
                    b'0'..=b'9' => {
                        bg_color = bg_color * 10 + c - b'0';
                        output_color(output, false, bg_color);
                        self.state = IRCatState::Normal;
                    }
                    _ => {
                        output_color(output, false, bg_color);
                        output.push(*c);
                        self.state = IRCatState::Normal;
                    }
                },
            }
        }
    }
}

fn output_color(output: &mut Vec<u8>, foreground: bool, color: u8) {
    output.extend_from_slice(b"\x1b[");
    output.push(if foreground { b'3' } else { b'4' });
    output.extend_from_slice(lookup_irc_color(color).as_bytes());
    output.push(b'm');
}

fn lookup_irc_color(color: u8) -> &'static str {
    match color {
        0 => "7",
        1 => "8;5;235",
        2 => "4",
        3 => "2",
        4 => "1",
        5 => "8;5;52",
        6 => "5",
        7 => "8;5;209",
        8 => "3",
        9 => "8;5;47",
        10 => "6",
        11 => "6",
        12 => "8;5;56",
        13 => "8;5;200",
        14 => "8;5;241",
        15 => "7",
        _ => todo!("missing color: {}", color),
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use crate::ircat;

    macro_rules! tests {
        ($(($name: ident, $input: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    let mut result = Vec::new();
                    ircat(&mut BufReader::new($input.as_ref()), &mut result).unwrap();
                    assert_eq!(result, $expected);
                }
            )*
        }
    }

    tests!(
        (none, b"foo bar", b"foo bar"),
        (empty, b"", b""),
        (
            newline,
            b"\x032blue\nnone\n",
            b"\x1b[34mblue\x1b[39m\x1b[49m\nnone\n"
        ),
        (
            stop_color,
            b"\x032blue\x03none\n",
            b"\x1b[34mblue\x1b[39m\x1b[49mnone\n"
        ),
        (two_digit_fg, b"\x0310cyan", b"\x1b[36mcyan"),
        (number_following, b"\x03002white", b"\x1b[37m2white"),
        (bg, b"\x032,3test", b"\x1b[34m\x1b[42mtest"),
        (bg_two_digit, b"\x032,10test", b"\x1b[34m\x1b[46mtest"),
        (no_fg, b"\x03,2invalid", b",2invalid"),
        (
            no_fg_in_color,
            b"\x032blue\x03,2invalid",
            b"\x1b[34mblue\x1b[39m\x1b[49m,2invalid"
        ),
        (
            double_reset,
            b"\x032blue\x03none\x03none",
            b"\x1b[34mblue\x1b[39m\x1b[49mnonenone"
        ),
    );

    #[test]
    #[should_panic]
    fn unknown_color() {
        ircat(&mut BufReader::new(b"\x0316text".as_ref()), &mut Vec::new()).unwrap();
    }
}
