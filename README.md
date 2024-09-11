# gm6020_can
This library controls a DJI gm6020 motor over the CAN bus. See [`gm6020_ros`](https://github.com/mjforan/gm6020_ros) for more details on setup and usage.

# Why Rust?
I wanted to try something new and Rust seems like a good skill to have. Feel free to give feedback on my code by submitting an issue or PR.

# Build
First, edit [the example](examples/gm6020_can_test.rs) and change the SocketCAN interface ("can0") and motor ID if necessary.
```
cargo build --release --examples
```

# Run
Make sure the motor is connected, powered on, and in voltage control mode.
```
./target/release/examples/gm6020_can_test
```

TODO gif of example running