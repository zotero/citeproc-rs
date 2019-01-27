use super::definition::*;

pub trait MutVisitor {
    fn visit_block(&mut self, block: &mut Block) {
        self.walk_block(block)
    }
    fn visit_attr(&mut self, attr: &mut Attr) {
        self.walk_attr(attr)
    }
    fn visit_inline(&mut self, inline: &mut Inline) {
        self.walk_inline(inline)
    }
    fn visit_meta(&mut self, _key: &str, meta: &mut MetaValue) {
        self.walk_meta(meta)
    }
    fn visit_vec_block(&mut self, vec_block: &mut Vec<Block>) {
        self.walk_vec_block(vec_block)
    }
    fn visit_vec_inline(&mut self, vec_inline: &mut Vec<Inline>) {
        self.walk_vec_inline(vec_inline)
    }
    fn walk_meta(&mut self, meta: &mut MetaValue) {
        use super::definition::MetaValue::*;
        match *meta {
            MetaMap(ref mut c) => {
                for (key, meta) in c {
                    self.visit_meta(key, meta);
                }
            }
            MetaList(ref mut c) => {
                for meta in c {
                    self.walk_meta(meta);
                }
            }
            MetaBool(_) => {}
            MetaString(_) => {}
            MetaInlines(ref mut v_inline) => {
                self.visit_vec_inline(v_inline);
            }
            MetaBlocks(ref mut v_block) => {
                self.visit_vec_block(v_block);
            }
        }
    }
    fn walk_pandoc(&mut self, pandoc: &mut Pandoc) {
        for (key, meta) in &mut (pandoc.0).0 {
            self.visit_meta(key, meta);
        }
        self.visit_vec_block(&mut pandoc.1);
    }
    fn walk_block(&mut self, block: &mut Block) {
        use super::definition::Block::*;
        match *block {
            Plain(ref mut vec_inline) | Para(ref mut vec_inline) => {
                self.visit_vec_inline(vec_inline);
            }
            LineBlock(ref mut vec_vec_inline) => {
                for vec_inline in vec_vec_inline {
                    self.visit_vec_inline(vec_inline);
                }
            }
            CodeBlock(ref mut attr, _) => self.visit_attr(attr),
            RawBlock { .. } => {}
            BlockQuote(ref mut vec_block) => {
                self.visit_vec_block(vec_block);
            }
            OrderedList(_, ref mut vec_vec_block) | BulletList(ref mut vec_vec_block) => {
                for vec_block in vec_vec_block {
                    self.visit_vec_block(vec_block);
                }
            }
            DefinitionList(ref mut c) => {
                for def in c {
                    self.visit_vec_inline(&mut def.0);
                    for vec_block in &mut def.1 {
                        self.visit_vec_block(vec_block);
                    }
                }
            }
            Header(_, ref mut attr, ref mut vec_inline) => {
                self.visit_attr(attr);
                self.visit_vec_inline(vec_inline);
            }
            HorizontalRule => {}
            Table(ref mut vec_inline, _, _, ref mut vv_block, ref mut vvv_block) => {
                self.visit_vec_inline(vec_inline);
                for vec_block in vv_block {
                    self.visit_vec_block(&mut vec_block.0);
                }
                for vv_block in vvv_block {
                    for vec_block in vv_block {
                        self.visit_vec_block(&mut vec_block.0);
                    }
                }
            }
            Div(ref mut attr, ref mut vec_block) => {
                self.visit_attr(attr);
                self.visit_vec_block(vec_block);
            }
            Null => {}
        }
    }
    fn walk_attr(&mut self, _attr: &mut Attr) {}
    fn walk_inline(&mut self, inline: &mut Inline) {
        use super::definition::Inline::*;
        match *inline {
            Str { .. } => {}
            Emph(ref mut c)
            | Strong(ref mut c)
            | Strikeout(ref mut c)
            | Superscript(ref mut c)
            | Subscript(ref mut c)
            | SmallCaps(ref mut c)
            | Quoted(_, ref mut c) => {
                self.visit_vec_inline(c);
            }
            Cite(ref mut v_cite, ref mut v_inl) => {
                for cite in v_cite {
                    self.visit_vec_inline(&mut cite.citation_prefix);
                    self.visit_vec_inline(&mut cite.citation_suffix);
                }
                self.visit_vec_inline(v_inl);
            }
            Code(ref mut attr, _) => self.visit_attr(attr),
            Space { .. } => {}
            SoftBreak { .. } => {}
            LineBreak { .. } => {}
            Math { .. } => {}
            RawInline { .. } => {}
            Link(ref mut attr, ref mut v_inline, _)
            | Image(ref mut attr, ref mut v_inline, _)
            | Span(ref mut attr, ref mut v_inline) => {
                self.visit_attr(attr);
                self.visit_vec_inline(v_inline);
            }
            Note(ref mut c) => {
                self.visit_vec_block(c);
            }
        }
    }
    fn walk_vec_block(&mut self, vec_block: &mut Vec<Block>) {
        for block in vec_block {
            self.visit_block(block);
        }
    }
    fn walk_vec_inline(&mut self, vec_inline: &mut Vec<Inline>) {
        for inline in vec_inline {
            self.visit_inline(inline);
        }
    }
}
