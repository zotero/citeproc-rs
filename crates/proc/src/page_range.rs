use csl::PageRangeFormat;

/// Returns the second number with the page range format applied.
pub fn truncate_prf(prf: PageRangeFormat, first: u32, mut second: u32) -> u32 {
    second = expand(first, second);
    match prf {
        PageRangeFormat::Chicago => {
            let mod100 = first % 100;
            let delta = second - first;
            if first < 100 || mod100 == 0 {
                second
            } else if mod100 < 10 && delta < 90 {
                truncate_diff(first, second, 1)
            } else if closest_smaller_power_of_10(first) == 1000 {
                let chopped = truncate_diff(first, second, 2);
                if closest_smaller_power_of_10(chopped) == 100 {
                    // force 4 digits if 3 are different
                    return truncate_diff(first, second, 4);
                }
                chopped
            } else {
                truncate_diff(first, second, 2)
            }
        }
        PageRangeFormat::Minimal => truncate_diff(first, second, 1),
        PageRangeFormat::MinimalTwo => truncate_diff(first, second, 2),
        PageRangeFormat::Expanded => second,
    }
}

#[test]
fn page_range_chicago() {
    fn go(a: u32, b: u32) -> u32 {
        truncate_prf(PageRangeFormat::Chicago, a, b)
    }
    // https://docs.citationstyles.org/en/stable/specification.html#appendix-v-page-range-formats
    // 1
    assert_eq!(go(3, 10), 10);
    assert_eq!(go(71, 72), 72);
    // 2
    assert_eq!(go(100, 104), 104);
    assert_eq!(go(600, 613), 613);
    assert_eq!(go(1100, 1123), 1123);
    // 3
    assert_eq!(go(101, 108), 8);
    assert_eq!(go(107, 108), 8);
    assert_eq!(go(505, 517), 17);
    assert_eq!(go(1002, 1006), 6);
    // 4
    assert_eq!(go(321, 325), 25);
    assert_eq!(go(415, 532), 532);
    assert_eq!(go(11564, 11568), 68);
    assert_eq!(go(13792, 13803), 803);
    // 5 (force 4 digits where 3 are different)
    assert_eq!(go(1496, 1504), 1504);
    assert_eq!(go(2787, 2816), 2816);
    // but if only two digits different, don't
    assert_eq!(go(1486, 1496), 96);
}

#[test]
fn test_truncate_diff() {
    assert_eq!(truncate_diff(101, 105, 1), 5);
    assert_eq!(truncate_diff(121, 125, 1), 5);
    assert_eq!(truncate_diff(121, 125, 2), 25);
    assert_eq!(truncate_diff(121, 125, 3), 125);
}

fn truncate_diff(a: u32, b: u32, min: u32) -> u32 {
    if b < a {
        return b;
    }
    let mut diff_started = false;
    let mut acc = 0u32;
    let mut iter_a = DigitsBase10::new(a);
    let mut iter_b = DigitsBase10::new(b);
    // fast forward iter_a until they have the same mask i.e. same remaining digit length
    while iter_a.mask > iter_b.mask {
        iter_a.next();
    }
    while iter_b.mask > iter_a.mask {
        if let Some(b_dig) = iter_b.next() {
            diff_started = true;
            acc *= 10;
            acc += b_dig as u32;
        }
    }
    let min_mask = 10_u32.pow(min);
    if iter_a.mask * 10 == min_mask {
        diff_started = true;
    }
    // Primitive zip so we can keep access to iter_a
    while let (Some(a_dig), Some(b_dig)) = (iter_a.next(), iter_b.next()) {
        if diff_started || a_dig != b_dig {
            diff_started = true;
            acc *= 10;
            acc += b_dig as u32;
        }
        if iter_a.mask * 10 == min_mask {
            diff_started = true;
        }
    }
    acc
}

#[test]
fn test_expand() {
    assert_eq!(expand(103, 4), 104);
    assert_eq!(expand(133, 4), 134);
    assert_eq!(expand(133, 54), 154);
}

fn expand(a: u32, b: u32) -> u32 {
    let mask = closest_smaller_power_of_10(b) * 10;
    (a - (a % mask)) + (b % mask)
}

// Thanks to timotree3 on the Rust users forum for writing this already
// https://users.rust-lang.org/t/iterate-through-digits-of-a-number/34465/9

pub struct DigitsBase10 {
    mask: u32,
    num: u32,
}

impl Iterator for DigitsBase10 {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.mask == 0 {
            return None;
        }

        let digit = self.num / self.mask % 10;
        self.mask /= 10;

        Some(digit as u8)
    }
}

fn closest_smaller_power_of_10(num: u32) -> u32 {
    let answer = 10_f64.powf((num as f64).log10().floor()) as u32;

    // these properties need to hold. I think they do, but the float conversions
    // might mess things up...
    debug_assert!(answer <= num);
    debug_assert!(answer > num / 10);
    answer
}

impl DigitsBase10 {
    pub fn new(num: u32) -> Self {
        let mask = if num == 0 {
            1
        } else {
            closest_smaller_power_of_10(num)
        };

        DigitsBase10 { mask, num }
    }
}
