# gm6020_can
This library controls a DJI gm6020 motor over the CAN bus. See [`gm6020_ros`](https://github.com/mjforan/gm6020_ros) for more details on setup and usage.

# Why Rust?
I wanted to try something new and Rust seems like a good skill to have. Feel free to give feedback on my code by submitting an issue or PR.

# Build
First, edit [the c++ example](examples/gm6020_can_test_cpp.rs) or [the rust example](gm6020_can/examples/gm6020_can_test.rs) and change the SocketCAN interface ("can0") and motor ID if necessary.
```
cargo build --examples
```

# Run
Make sure the motor is connected, powered on, and in voltage control mode.
```
# C++ example
cargo run --example gm6020_can_test_cpp

# Rust example
cd gm6020_can
cargo run --example gm6020_can_test
```

TODO gif of example running