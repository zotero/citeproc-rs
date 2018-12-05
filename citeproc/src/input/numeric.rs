/// Tests whether the given variables (Appendix IV - Variables) contain numeric content. Content is
/// considered numeric if it solely consists of numbers. Numbers may have prefixes and suffixes
/// (“D2”, “2b”, “L2d”), and may be separated by a comma, hyphen, or ampersand, with or without
/// spaces (“2, 3”, “2-4”, “2 & 4”). For example, “2nd” tests “true” whereas “second” and “2nd
/// edition” test “false”.

// We want to parse:
//
// 2, 4         => Num(2) Comma Num(4)
// 2-4, 5       => Num(2) Hyphen Num(4) Comma Num(5)
// 2-4, 5       => Num(2) Hyphen Num(4) Comma Num(5)
// 2nd          => Aff("",  2, "nd")
// L2tp         => Aff("L", 2, "tp")
// 2nd-4th      => Aff("",  2, "nd") Hyphen Aff("", 4, "th")
//
// We don't want to parse:
//
// 2nd edition  => Aff(2, "nd") ... Err("edition") -> not numeric
// -5           => Err("-5") -> not numeric

pub enum NumericToken<'r> {
    Num(i32),
    Affixed(&'r str, i32, &'r str),
    Sep(&'r str),
}

pub enum NumericValue<'r> {
    // for values arriving as actual integers
    Int(i32),
    // for values that were originally strings, and maybe got parsed into numbers as an alternative
    Parsed(&'r str, Option<i32>),
}

impl<'r> NumericValue<'r> {
    pub fn numeric(&self) -> Option<i32> {
        match *self {
            NumericValue::Int(i) => Some(i),
            NumericValue::Parsed(_, oi) => oi,
        }
    }
    pub fn to_string(&self) -> String {
        match *self {
            NumericValue::Int(i) => format!("{}", i),
            NumericValue::Parsed(s, _) => s.to_owned(),
        }
    }
}

// impl<'r> From<&'r str> for NumericValue<'r> {
//     pub fn from<'a>(value: &'a str) -> Result<Self, &'a str> {
//         let int = value.parse::<i32>().map_err(|| );
//         Ok(0)
//     }
// }

