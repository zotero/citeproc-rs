use super::ir::*;
use super::Proc;
use crate::input::DateOrRange;
use crate::input::Reference;
use crate::output::OutputFormat;
use crate::style::element::{Date as DateEl, Formatting};

impl<'s> Proc<'s> for DateEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        let content = refr
            .date
            .get(&self.variable)
            .and_then(|val| {
                if let DateOrRange::Single(d) = val {
                    Some(d)
                } else {
                    None
                }
            })
            .map(|val| {
                let string = format!("{}-{}-{}", val.year, val.month, val.day);
                fmt.group(
                    &[
                        fmt.plain(&self.affixes.prefix),
                        fmt.text_node(&string, &self.formatting),
                        fmt.plain(&self.affixes.suffix),
                    ],
                    "",
                    &Formatting::default(),
                )
            });
        IR::Rendered(content)
    }
}
