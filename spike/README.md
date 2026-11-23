# FRAS technical spike (throwaway)

Milestone 0 of Fire, Rust and Steam: code written to find out what Horizon's mod API can do for a shaft-power
mod. It was not released and is not the start of the real mod. The results are in [FINDINGS.md](FINDINGS.md).

The spike's code was removed from the repository when 0.1 was built: a native mod's package is the repository
root, and a second crate inside it (`spike/Cargo.toml`) makes the package invalid. The code is in the history,
at commit `5669c0d` (`git checkout 5669c0d -- spike` brings it back; its mod id is `fras_spike`).
