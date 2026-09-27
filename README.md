# strata-json

A JSON parser written in Rust, with no dependencies or unsafe code. Unescaped
strings borrow from the input, and numbers keep their original text so large
integers don't lose precision during parsing.

Requires Rust 1.85 or newer. Not published to crates.io yet.

## Usage

Clone this repo next to your project and add:

```toml
[dependencies]
strata-json = { path = "../strata-json" }
```

```rust
use strata_json::{from_str, Error};

fn main() -> Result<(), Error> {
    let input = r#"{"name":"Ada","age":37,"tags":["rust","json"]}"#;
    let value = from_str(input)?;

    assert_eq!(value.get("name").and_then(|v| v.as_str()), Some("Ada"));

    let age = value.get("age")
        .and_then(|v| v.as_number())
        .and_then(|n| n.as_u64());
    assert_eq!(age, Some(37));
    Ok(())
}
```

Use `from_slice(&bytes)` for a UTF-8 byte buffer. The input must stay alive while
you use the parsed value. Arrays and objects allocate, as do strings that need
escape decoding. Accessors return `None` for missing keys or the wrong type.

## A few details

- Objects keep their input order and duplicate keys. `get()` returns the last
  matching key with a linear scan; `as_object()` gives you every entry.
- Numbers are stored as validated text. `as_i64()` and `as_u64()` accept integer
  tokens within range, while `as_f64()` can round or underflow and returns `None`
  on overflow. Use `as_str()` when you need the exact number.
- Equality compares number spelling and object order: `1` and `1.0` are distinct.
- Strings support JSON escapes and UTF-16 surrogate pairs. Invalid UTF-8 and
  unpaired surrogates are rejected.
- Comments, trailing commas, leading zeros, a leading BOM, and trailing content
  are rejected. Any JSON value can be the root.
- Nesting is limited to 128 arrays/objects. `from_str_with_depth(input, limit)`
  lets you lower that limit; zero allows only scalars and higher limits are
  clamped to 128. For untrusted input, set your own input size limit too.

Errors include a category, zero-based byte offset, and one-based line and byte
column. For example:

```text
unexpected token at line 2, column 8 (byte 9)
```

This currently builds an in-memory value tree. There's no Serde integration,
serialization, streaming, or `no_std` support.

## Running it

```sh
cargo test
cargo run --example inspect -- examples/sample.json
cargo bench --bench parse
cargo doc --no-deps --open
```

Tests cover syntax errors, borrowing, numeric boundaries, nesting, mutated
inputs, and escape decoding for every Unicode scalar. CI also checks formatting
and Clippy.

The benchmark uses generated objects, escaped strings, and integer arrays. It
reports median throughput across five samples, including parsing and dropping
the result. These are warm-cache microbenchmarks; there's no comparison against
other parsers yet.

## TODO

- Add a benchmark using a larger, more varied JSON document.
- Measure allocations alongside throughput.
- Let the inspect example read from stdin.

## License

[MIT](LICENSE)
