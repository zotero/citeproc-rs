#[cfg(feature="flame_it")]
use std::mem;

#[cfg(feature="flame_it")]
use std::fs::File;


/// Implementation as given by @ebarnard at: https://github.com/TyOverby/flame/issues/33#issuecomment-352312506
#[cfg(feature="flame_it")]
pub fn write_flamegraph(path: &str) {
    let mut spans = flame::threads().into_iter().next().unwrap().spans;
    merge_spans(&mut spans);
    flame::dump_html_custom(&mut File::create(path).unwrap(), &spans)
        .unwrap();
}

#[cfg(feature="flame_it")]
fn merge_spans(spans: &mut Vec<flame::Span>) {
    if spans.is_empty() {
        return;
    }

    // Sort so spans to be merged are adjacent and spans with the most children are
    // merged into to minimise allocations.
    spans.sort_unstable_by(|s1, s2| {
        let a = (&s1.name, s1.depth, usize::max_value() - s1.children.len());
        let b = (&s2.name, s2.depth, usize::max_value() - s2.children.len());
        a.cmp(&b)
    });

    // Copy children and sum delta from spans to be merged
    let mut merge_targets = vec![0];
    {
        let mut spans_iter = spans.iter_mut().enumerate();
        let (_, mut current) = spans_iter.next().unwrap();
        for (i, span) in spans_iter {
            if current.name == span.name && current.depth == span.depth {
                current.delta += span.delta;
                let children = mem::replace(&mut span.children, Vec::new());
                current.children.extend(children.into_iter());
            } else {
                current = span;
                merge_targets.push(i);
            }
        }
    }

    // Move merged spans to the front of the spans vector
    for (target_i, &current_i) in merge_targets.iter().enumerate() {
        spans.swap(target_i, current_i);
    }

    // Remove duplicate spans
    spans.truncate(merge_targets.len());

    // Merge children of the newly collapsed spans
    for span in spans {
        merge_spans(&mut span.children);
    }
}


#[cfg(not(feature="flame_it"))]
pub fn write_flamegraph(path: &str) {
}
