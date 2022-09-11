use crate::source_analysis::prelude::*;
use syn::*;

impl SourceAnalysis {
    pub(crate) fn check_attr_list(&mut self, attrs: &[Attribute], ctx: &Context) -> bool {
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let mut check_cover = true;
        for attr in attrs {
            analysis.ignore_tokens(attr);
            if let Ok(x) = attr.parse_meta() {
                if check_cfg_attr(&x) {
                    check_cover = false;
                } else if x.path().is_ident("cfg") {
                    if let Meta::List(ref ml) = x {
                        let mut skip = false;
                        for c in &ml.nested {
                            if let NestedMeta::Meta(Meta::Path(ref i)) = c {
                                skip |= i.is_ident("test") && ctx.config.ignore_tests();
                            }
                        }
                        if skip {
                            check_cover = false;
                        }
                    }
                }
            }
            if !check_cover {
                break;
            }
        }
        check_cover
    }
}

pub(crate) fn check_cfg_attr(attr: &Meta) -> bool {
    let mut ignore_span = false;
    let id = attr.path();

    if id.is_ident("no_coverage") {
        ignore_span = true;
    } else if id.is_ident("cfg") {
        if let Meta::List(ml) = attr {
            'outer: for p in ml.nested.iter() {
                if let NestedMeta::Meta(Meta::List(ref i)) = p {
                    if i.path.is_ident("not") {
                        for n in i.nested.iter() {
                            if let NestedMeta::Meta(Meta::Path(ref pth)) = n {
                                if pth.is_ident("tarpaulin_include") || pth.is_ident("tarpaulin") {
                                    ignore_span = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        let skip_attrs = vec!["tarpaulin", "skip"];
        let mut n = 0;
        ignore_span = true;
        for (segment, attr) in id.segments.iter().zip(skip_attrs.iter()) {
            n += 1;
            if segment.ident != attr {
                ignore_span = false;
            }
        }
        if n < skip_attrs.len() {
            ignore_span = false;
        }
    }
    ignore_span
}
