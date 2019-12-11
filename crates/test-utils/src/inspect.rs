use anyhow::{anyhow, Error};
use citeproc::prelude::*;

pub fn debug_gen4_flat(eng: &Processor, cite_id: CiteId) -> Result<String, Error> {
    let ir = eng.ir_gen4_conditionals(cite_id);
    let fmt = eng.get_formatter();
    let flat = ir.ir.flatten(&fmt).ok_or_else(|| anyhow!("flatten was none"))?;
    Ok(serde_sexpr::to_string(&flat)?)
}

pub fn debug_built_cluster(eng: &Processor, cluster: ClusterId) -> Result<String, Error> {
    Ok(serde_sexpr::to_string(&eng.built_cluster(cluster))?)
}
