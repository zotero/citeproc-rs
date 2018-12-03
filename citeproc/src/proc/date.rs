use super::ir::*;
use super::Proc;
use crate::input::DateOrRange;
use crate::input::Reference;
use crate::output::OutputFormat;
use crate::style::element::{Date as DateEl, Formatting};
use super::cite_context::*;

impl<'c, 's: 'c> Proc<'c, 's> for DateEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let content = ctx.reference.date
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
