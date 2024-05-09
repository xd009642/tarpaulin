use crate::source_analysis::prelude::*;
use syn::*;

mod predicates {
    pub fn is_test_attribute(id: &syn::Path) -> bool {
        id.segments
            .last()
            .unwrap()
            .ident
            .to_string()
            .ends_with("test")
    }
}

impl SourceAnalysis {
    pub(crate) fn check_attr_list(&mut self, attrs: &[Attribute], ctx: &Context) -> bool {
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let mut check_cover = true;
        for attr in attrs {
            analysis.ignore_tokens(attr);
            if check_cfg_attr(&attr.meta) {
                check_cover = false;
            } else if attr.meta.path().is_ident("cfg") {
                let mut skip = false;
                attr.parse_nested_meta(|meta| {
                    skip |= meta.path.is_ident("test") && !ctx.config.include_tests();
                    Ok(())
                });
                if skip {
                    check_cover = false;
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
            let mut is_not = false;
            let _ = ml.parse_nested_meta(|nested| {
                if is_not {
                    ignore_span |= nested.path.is_ident("tarpaulin_include")
                        || nested.path.is_ident("tarpaulin");
                } else {
                    is_not = nested.path.is_ident("not");
                }
                Ok(())
            });
        }
    } else if id.is_ident("cfg_attr") {
        if let Meta::List(ml) = attr {
            let tarp_cfged_ignores = &["no_coverage"];
            let mut first = true;
            let mut is_tarpaulin = false;
            ml.parse_nested_meta(|nested| {
                if first && nested.path.is_ident("tarpaulin") {
                    first = false;
                    is_tarpaulin = true;
                } else if !first && is_tarpaulin {
                    ignore_span |= tarp_cfged_ignores.iter().any(|x| nested.path.is_ident(x));
                }
                Ok(())
            });
        }
    } else if predicates::is_test_attribute(id) {
        ignore_span = true;
    } else {
        let skip_attrs = &["tarpaulin", "skip"];
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
