# FRAS technical spike (throwaway)

Milestone 0 of Fire, Rust and Steam: code written to find out what Horizon's mod API can do for a shaft-power
mod. **It is not released and is not the start of the real mod**: the real code is written after each
subsystem's spec. The results are in [FINDINGS.md](FINDINGS.md).

```sh
cargo test                                         # the network model (src/network.rs)
horizon-launcher native-build . --trust-native-code   # build it into the selected instance
```

Its mod id is `fras_spike`, so a test world never mixes with the real `fras`.
