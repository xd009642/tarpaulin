use crate::source_analysis::prelude::*;
use quote::ToTokens;
use std::cmp::{max, min};
use std::ops::Range;
use syn::{spanned::Spanned, *};

impl SourceAnalysis {
    pub(crate) fn visit_macro_call(&mut self, mac: &Macro, ctx: &Context) -> SubResult {
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let mut skip = false;
        if let Some(PathSegment {
            ref ident,
            arguments: _,
        }) = mac.path.segments.last()
        {
            let ident_s = ident.to_string();
            let unreachable = ident == "unreachable";
            let standard_ignores =
                ident == "unimplemented" || ident == "include" || ident == "cfg" || ident == "todo";
            let ignore_panic = ctx.config.ignore_panics
                && (ident == "panic"
                    || ident_s.starts_with("assert")
                    || ident_s.starts_with("debug_assert"));
            if standard_ignores || ignore_panic || unreachable {
                analysis.ignore_tokens(mac);
                skip = true;
            }
            if unreachable {
                return SubResult::Unreachable;
            }
        }
        if !skip {
            analysis.cover_logical_line(mac.span());
        }
        SubResult::Ok
    }
}

pub(crate) fn get_line_range<T>(tokens: T) -> Range<usize>
where
    T: ToTokens,
{
    let mut start = None;
    let mut end = None;
    for token in tokens.into_token_stream() {
        let temp_start = token.span().start().line;
        let temp_end = token.span().end().line + 1;
        start = match start {
            Some(x) => Some(min(temp_start, x)),
            None => Some(temp_start),
        };
        end = match end {
            Some(x) => Some(max(temp_end, x)),
            None => Some(temp_end),
        };
    }
    match (start, end) {
        (Some(s), Some(e)) => s..e,
        _ => 0..0,
    }
}
