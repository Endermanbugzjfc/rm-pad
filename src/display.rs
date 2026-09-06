use core::fmt;
use std::str::FromStr;

use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
pub enum SizeData {
    AspectRatio(AspectRatio),
    Resolution(Resolution),
}

impl SizeData {
    pub fn get_width(&self) -> u32 {
        match *self {
            Self::AspectRatio(ratio) => ratio.get_width(),
            Self::Resolution(res) => res.get_width(),
        }
    }

    pub fn get_height(&self) -> u32 {
        match *self {
            Self::AspectRatio(ratio) => ratio.get_height(),
            Self::Resolution(res) => res.get_height(),
        }
    }
}

impl fmt::Display for SizeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AspectRatio(ratio) => ratio.fmt(f),
            Self::Resolution(res) => res.fmt(f),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "&str")]
pub struct AspectRatio(u32, u32);

impl AspectRatio {
    pub fn new(w: u32, h: u32) -> Self {
        let hcf = find_hcf(w, h);
        Self(w / hcf, h / hcf)
    }

    pub fn get_width(&self) -> u32 {
        self.0
    }

    pub fn get_height(&self) -> u32 {
        self.1
    }
}

impl fmt::Display for AspectRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

impl TryFrom<&str> for AspectRatio {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match match_two_non_zero_u32(s, ':') {
            Some((w, h)) => Ok(Self::new(w, h)),
            None => Err(format!(
                "Invalid aspect ratio '{}'. Valid format: 'WIDTH:HEIGHT'. Valid range: non-zero positive 32 bit integers",
                s
            ))
        }
    }
}

impl FromStr for AspectRatio {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "&str")]
pub struct Resolution(u32, u32);

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self(width, height)
    }

    pub fn get_width(&self) -> u32 {
        self.0
    }

    pub fn get_height(&self) -> u32 {
        self.1
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.0, self.1)
    }
}

impl TryFrom<&str> for Resolution {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match match_two_non_zero_u32(&s.to_lowercase(), 'x') {
            Some((w, h)) => Ok(Self::new(w, h)),
            None => Err(format!(
                "Invalid resolution '{}'. Valid format: 'WIDTHxHEIGHT'. Valid range: non-zero positive 32 bit integers",
                s
            ))
        }
    }
}

impl FromStr for Resolution {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

fn match_two_non_zero_u32(input: &str, delimiter: char) -> Option<(u32, u32)> {
    let clean_input = input.replace(char::is_whitespace, "");
    let ratio_input: Vec<&str> = clean_input.split(delimiter).collect();
    if let [w_input, h_input] = ratio_input.as_slice() {
        let w_parse = w_input.parse::<u32>().ok().filter(|n| *n != 0);
        let h_parse = h_input.parse::<u32>().ok().filter(|n| *n != 0);
        if let Some((w, h)) = w_parse.zip(h_parse) {
            return Some((w, h));
        }
    }

    None
}

/// Finds the highest common factor (also knowns as the greatest common divisor).
fn find_hcf<T>(mut a: T, mut b: T) -> T
where
    T: PartialEq + From<u8> + Copy + std::ops::Rem<Output = T>
{
    while b != 0.into() {
        let tmp = b;
        b = a % b;
        a = tmp;
    }

    a
}
