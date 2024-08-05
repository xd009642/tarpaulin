use crate::source_analysis::prelude::*;
use syn::*;

pub mod predicates {
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
                let _ = attr.parse_nested_meta(|meta| {
                    skip |=
                        predicates::is_test_attribute(&meta.path) && !ctx.config.include_tests();
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
    tracing::trace!("cfg attr: {}", attr.to_token_stream());
    let mut ignore_span = false;
    let id = attr.path();

    // no coverage is now deprecated in the compiler, so in future we can remove this just to
    // minimise some of this code
    if id.is_ident("no_coverage") {
        ignore_span = true;
    } else if id.is_ident("coverage") {
        if let Meta::List(ml) = attr {
            let _ = ml.parse_nested_meta(|nested| {
                ignore_span |= nested.path.is_ident("off");
                Ok(())
            });
        }
    } else if id.is_ident("cfg") {
        if let Meta::List(ml) = attr {
            let _ = ml.parse_nested_meta(|nested| {
                if nested.path.is_ident("not") {
                    nested.parse_nested_meta(|meta| {
                        ignore_span |= meta.path.is_ident("tarpaulin_include")
                            || meta.path.is_ident("tarpaulin");
                        Ok(())
                    })
                } else {
                    Ok(())
                }
            });
        }
    } else if id.is_ident("cfg_attr") {
        if let Meta::List(ml) = attr {
            let mut first = true;
            let mut is_tarpaulin = false;
            let _ = ml.parse_nested_meta(|nested| {
                if first && nested.path.is_ident("tarpaulin") {
                    first = false;
                    is_tarpaulin = true;
                } else if !first && is_tarpaulin {
                    if nested.path.is_ident("no_coverage") {
                        ignore_span = true;
                    } else if nested.path.is_ident("coverage") {
                        let _ = nested.parse_nested_meta(|nested| {
                            ignore_span |= nested.path.is_ident("off");
                            Ok(())
                        });
                    }
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
