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
            } else if ctx.config.ignore_tests && x.path().is_ident("cfg") {
                if let Meta::List(ref ml) = x {
                    let mut skip = false;
                    for c in &ml.nested {
                        if let NestedMeta::Meta(Meta::Path(ref i)) = c {
                            skip |= i.is_ident("test");
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

pub(crate) fn check_cfg_attr(attr: &Meta) -> bool {
    let mut ignore_span = false;
    let id = attr.path();
    if id.is_ident("cfg_attr") {
        if let Meta::List(ml) = attr {
            let mut skip_match = false;
            let list = vec!["tarpaulin", "skip"];
            for (p, x) in ml.nested.iter().zip(list.iter()) {
                match p {
                    NestedMeta::Meta(Meta::Path(ref i)) => {
                        skip_match = i.is_ident(x);
                    }
                    _ => skip_match = false,
                }
                if !skip_match {
                    break;
                }
            }
            ignore_span = skip_match;
        }
    }
    ignore_span
}
