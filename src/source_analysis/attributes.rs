use crate::source_analysis::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use syn::punctuated::Punctuated;
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
                if eval_cfg_attr(&attr.meta, ctx) == Some(false) {
                    check_cover = false;
                }
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

fn eval_cfg_attr(attr: &Meta, ctx: &Context<'_>) -> Option<bool> {
    let Meta::List(list) = attr else {
        return None;
    };
    let predicates = list
        .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .ok()?;
    match predicates.len() {
        0 => None,
        1 => predicates.first().and_then(|meta| eval_cfg(meta, ctx)),
        _ => eval_all(predicates.iter(), ctx),
    }
}

fn eval_cfg(meta: &Meta, ctx: &Context<'_>) -> Option<bool> {
    if meta.path().is_ident("all") {
        let Meta::List(list) = meta else {
            return None;
        };
        let predicates = list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        eval_all(predicates.iter(), ctx)
    } else if meta.path().is_ident("any") {
        let Meta::List(list) = meta else {
            return None;
        };
        let predicates = list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        eval_any(predicates.iter(), ctx)
    } else if meta.path().is_ident("not") {
        let Meta::List(list) = meta else {
            return None;
        };
        let predicates = list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        if predicates.len() != 1 {
            return None;
        }
        predicates
            .first()
            .and_then(|predicate| eval_cfg(predicate, ctx))
            .map(|result| !result)
    } else if meta.path().is_ident("feature") {
        let Meta::NameValue(name_value) = meta else {
            return None;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(feature),
            ..
        }) = &name_value.value
        else {
            return None;
        };
        Some(active_features(ctx).contains(&feature.value()))
    } else if meta.path().is_ident("test") {
        Some(ctx.config.include_tests())
    } else if meta.path().is_ident("tarpaulin") {
        Some(!ctx.config.avoid_cfg_tarpaulin)
    } else {
        None
    }
}

fn eval_all<'a>(predicates: impl Iterator<Item = &'a Meta>, ctx: &Context<'_>) -> Option<bool> {
    let mut result = Some(true);
    for predicate in predicates {
        match eval_cfg(predicate, ctx) {
            Some(false) => return Some(false),
            Some(true) => {}
            None => result = None,
        }
    }
    result
}

fn eval_any<'a>(predicates: impl Iterator<Item = &'a Meta>, ctx: &Context<'_>) -> Option<bool> {
    let mut result = Some(false);
    for predicate in predicates {
        match eval_cfg(predicate, ctx) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => result = None,
        }
    }
    result
}

fn active_features(ctx: &Context<'_>) -> HashSet<String> {
    let mut active = HashSet::new();
    let metadata = ctx.config.get_metadata();
    let package = metadata
        .as_ref()
        .and_then(|metadata| package_for_file(ctx.file, metadata.packages.iter()));

    let Some(package) = package else {
        return active;
    };

    if ctx.config.all_features {
        active.extend(package.features.keys().cloned());
        return active;
    }

    if !ctx.config.no_default_features {
        add_feature("default", &package.features, &mut active);
    }

    if let Some(features) = ctx.config.features.as_ref() {
        for feature in features.split_ascii_whitespace() {
            let feature = feature
                .rsplit_once('/')
                .map_or(feature, |(_, feature)| feature);
            add_feature(feature, &package.features, &mut active);
        }
    }

    active
}

fn package_for_file<'a>(
    file: &Path,
    packages: impl Iterator<Item = &'a cargo_metadata::Package>,
) -> Option<&'a cargo_metadata::Package> {
    packages
        .filter_map(|package| {
            let manifest = PathBuf::from(package.manifest_path.as_std_path());
            let package_root = manifest.parent()?;
            file.starts_with(package_root)
                .then_some((package, package_root.components().count()))
        })
        .max_by_key(|(_, package_root_len)| *package_root_len)
        .map(|(package, _)| package)
}

fn add_feature(
    feature: &str,
    features: &std::collections::BTreeMap<String, Vec<String>>,
    active: &mut HashSet<String>,
) {
    if !features.contains_key(feature) || !active.insert(feature.to_string()) {
        return;
    }

    for dependency in &features[feature] {
        let dependency_feature = dependency
            .strip_prefix("dep:")
            .unwrap_or(dependency)
            .split_once('?')
            .map_or(dependency.as_str(), |(_, feature)| feature)
            .split_once('/')
            .map_or(dependency.as_str(), |(_, feature)| feature);

        if features.contains_key(dependency_feature) {
            add_feature(dependency_feature, features, active);
        }
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
