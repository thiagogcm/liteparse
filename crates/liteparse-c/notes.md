# Core-side changes that would streamline this binding

Deferred from a review of the C-binding commit to keep it from touching the
core crate. Each item removes duplication or waste that currently lives in
`src/lib.rs`.

1. **Shared config validation.** Promote the cross-field rule in
   `LiteParse::validate_output_config` (`crates/liteparse/src/parser.rs`) to a
   public `LiteParseConfig::validate()`. `validate_config_semantics` here is a
   hand-copy of that rule, and the error messages have already drifted
   (`(or image_mode = embed)` vs `or image_mode = "embed"`). New core rules
   won't be enforced by the options builder until this is unified.

2. **Full-result JSON formatter in core.** Add optional, skip-serialized
   `text`/`creator`/`producer`/`doc_meta` fields to `ParseResultJson` plus a
   `format_json_result_full()` in `output/json.rs` (CLI output unchanged;
   fields stay `None` there). `format_result` here currently builds the same
   superset by mutating a `serde_json::Value` from `json_result_value`: the
   result is materialized three times (struct, then `Value` tree, then string), the
   full document text and `doc_meta` are cloned, and the "CLI schema + 4
   fields" shape exists only in this crate, untested against core schema
   evolution. Would also let this crate drop `json_result_value` (whose only
   caller is here) and its direct `serde_json` dependency.

3. **Borrowed byte input.** `PdfInput::Bytes(Vec<u8>)` forces
   `copy_input_bytes` to memcpy the caller's whole document even though
   `parse_bytes` is synchronous and the buffer necessarily outlives the call.
   A borrowed variant (e.g. `Cow<[u8]>`) or a `parse_slice` entry point would
   remove the copy and the doubled peak memory. The header documents "the
   input is copied", so relaxing that contract is a deliberate API decision.
