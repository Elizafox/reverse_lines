# reverse_lines

This library provides a small Rust Iterator for reading files line by line with a buffer in reverse.

It is a rework of [rev_lines](https://github.com/rev_lines/rev_lines).

### Documentation

Documentation is available on [Docs.rs](https://docs.rs/reverse_lines).

### Example

```rust
extern crate reverse_lines;

use reverse_lines::ReverseLines;

let file = File::open("/path/to/file").unwrap();
let mut reverse_lines = ReverseLines::new(file).unwrap();

for line in reverse_lines {
    println!("{}", line.unwrap());
}
```
