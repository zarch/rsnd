# rsnd
Raiplay sound CLI client


## Usage

```bash
❯ rsnd --help
Usage: rsnd [OPTIONS] --url <URL>

Options:
  -u, --url <URL>        URL of the HTML page
  -f, --folder <FOLDER>  Path to the local folder [default: .]
  -c, --cache <CACHE>    Path to the cache folder [default: /tmp]
  -h, --help             Print help
  -V, --version          Print version
```

Here an example:

```bash
❯ ./target/debug/rsnd --url https://www.raiplaysound.it/audiolibri/itremoschettieri \
--folder=libri/itremoschettieri \
--cache=cache
```
---

# rsnd
Raiplay Sound CLI Client

`rsnd` is a command-line tool designed to download audio content from Raiplay Sound.

## Features
- Fetches and downloads audio files from Raiplay Sound.
- Caches HTML pages and metadata to improve download efficiency.
- Allows specifying download and cache directories.

## Table of Contents
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install): Ensure you have Rust installed. You can install Rust using [rustup](https://rustup.rs/).

### Building from Source
Clone the repository and build the project using Cargo:

```bash
git clone https://github.com/zarch/rsnd.git
cd rsnd
cargo build --release
```

The compiled binary will be located in target/release/rsnd.

## Usage

```bash
❯ rsnd --help
Usage: rsnd [OPTIONS] --url <URL>

Options:
  -u, --url <URL>        URL of the HTML page
  -f, --folder <FOLDER>  Path to the local folder [default: .]
  -c, --cache <CACHE>    Path to the cache folder [default: /tmp]
  -h, --help             Print help
  -V, --version          Print version
```

## Example

To download the audiobook "I Tree Moschettieri":

```bash
❯ ./target/release/rsnd --url https://www.raiplaysound.it/audiolibri/itremoschettieri \
    --folder=libri/itremoschettieri \
    --cache=cache
```

This will download the audiobook files to `libri/itremoschettieri` and use cache as the cache directory.

## Contributing

Contributions are welcome! Please follow these steps to contribute:

1. Fork the repository.
2. Create a new branch (git checkout -b feature-branch).
3. Make your changes.
4. Ensure all tests pass (cargo test).
5. Format the code (cargo fmt).
6. Lint the code (cargo clippy).
7. Commit your changes (git commit -am 'Add new feature').
8. Push to the branch (git push origin feature-branch).
9. Create a new Pull Request.


### Running Tests

Run the tests using Cargo:

```bash
cargo test
```

### Code Formatting

Ensure your code is formatted according to Rust standards:

```bash
cargo fmt
```

### Linting
Check your code for common mistakes and improve readability:

```bash
cargo clippy
```

### Code Coverage

You can check the code coverage using grcov:

Install grcov:

```bash
cargo install grcov
```

Generate the coverage report:

```bash
export CARGO_INCREMENTAL=0
export RUSTFLAGS='-Cinstrument-coverage'
export LLVM_PROFILE_FILE='rsnd-%p-%m.profraw'
cargo build
cargo test
grcov . --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/coverage/
Open ./target/coverage/index.html to view the coverage report.
```

## License
This project is licensed under the Apache2/MIT License. See the LICENSE file for more details.
