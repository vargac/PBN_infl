# Influence Maximization in Partially Specified Boolean Networks

This repository contains two tools for analysis of partially specified
Boolean networks (PBNs). The tools require the input format .aeon. Some
models are in the `models` folder. You can find more models
[here](https://github.com/sybila/biodivine-boolean-models).
The codebase is written in Rust. To compile the source files,
navigate to the `pbn_ibmfa` directory and run `cargo run --release`.

## Command line tool

To run the command line tool, execute the `pbn_ibmfa` binary in a folder
`pbn_ibmfa/target/release`. Get help by `./pbn_ibmfa --help`. The output JSON
file produced by the command `simulation` may be illustrated by the Python3
script `scripts/plot_simulations.py`. `matplotlib` library is needed for that.
Just pass the path to the JSON file as the only argument for the script.

## Graphical tool

Run `server` in `pbn_ibmfa/target/release`. The address and the port number
may be specified, see `./server --help`. Then open `site/index.html` in Your
favorite web browser.

(For more information, see `text/main.tex` (or even compile it))
