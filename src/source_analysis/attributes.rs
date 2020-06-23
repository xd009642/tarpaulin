use crate::source_analysis::prelude::*;
use syn::*;

pub(crate) fn check_attr_list(
    attrs: &[Attribute],
    ctx: &Context,
    analysis: &mut LineAnalysis,
) -> bool {
    let mut check_cover = true;
    for attr in attrs {
        analysis.ignore_tokens(attr);
        if let Ok(x) = attr.parse_meta() {
            if check_cfg_attr(&x) {
                check_cover = false;
            } else if x.path().is_ident("cfg") {
                match x {
                    Meta::List(ref ml) => {
                        let mut skip = false;
                        for c in &ml.nested {
                            if let NestedMeta::Meta(Meta::Path(ref i)) = c {
                                skip |= i.is_ident("test") && ctx.config.ignore_tests;
                            }
                        }
                        if skip {
                            check_cover = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        if !check_cover {
            break;
        }
    }
    check_cover
}

pub(crate) fn check_cfg_attr(attr: &Meta) -> bool {
    let mut ignore_span = false;
    let id = attr.path();
    if id.is_ident("cfg_attr") {
        if let Meta::List(ml) = attr {
            let mut skip_match = false;
            let mut found_tarpaulin = false;
            for p in ml.nested.iter() {
                match p {
                    NestedMeta::Meta(Meta::Path(ref i)) => {
                        if !found_tarpaulin {
                            skip_match = i.is_ident("tarpaulin");
                            found_tarpaulin |= skip_match;
                        } else {
                            skip_match = i.is_ident("skip");
                        }
                    }
                    _ => skip_match = false,
                }
                if !skip_match {
                    break;
                }
            }
            ignore_span = skip_match;
        }
    } else if id.is_ident("cfg") {
        if let Meta::List(ml) = attr {
            'outer: for p in ml.nested.iter() {
                if let NestedMeta::Meta(Meta::List(ref i)) = p {
                    if i.path.is_ident("not") {
                        for n in i.nested.iter() {
                            if let NestedMeta::Meta(Meta::Path(ref pth)) = n {
                                if pth.is_ident("tarpaulin") {
                                    ignore_span = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    ignore_span
}
