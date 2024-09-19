# `gm6020_can`
This Rust library controls a DJI GM6020 motor over a Linux SocketCAN interface. Why Rust? I wanted to try something new and Rust seems like a good skill to have in 2024. Feel free to give feedback on my code by submitting an issue or PR.

# `gm6020_can_cpp`
This library provides a C/C++ wrapper over `gm6020_can`. Static and dynamic libraries are created in the target directory and header files are generated in the include directory. A neat way to include this in your C++ program is to use Corrosion, which will automatically build the Rust crate and create a CMake target to link against.

Unfortunately the C header does not contain "fully-qualified" names. Ideally each name would be prefixed with `gm6020_can_` to avoid conflict of common names like `init`. It is easy enough to generate these name prefixes in the header file but then they do not link to the Rust library. There is some ongoing work in the `cbindgen` tool to address this. If it is an issue for your project, change the function names in Rust and uncomment the `[export]` block in [`cbindgen_c.toml`](cbindgen_c.toml) to prefix all other items.

# [`gm6020_ros`](https://github.com/mjforan/gm6020_ros/)
I also wrote a ROS 2 wrapper over this library, which enables advanced control interfaces such as `ros2_control` and `MoveIt`. Talk about layers of abstraction! This repo also has an example hardware setup and `CMakeLists.txt`.

# Hardware
The GM6020 motor should be accessible via a SocketCAN interface. This can be accomplished with a USB CAN adapter, Raspberry Pi HAT, or a computer with a built-in CAN interface like an NVIDIA Orin. Don't forget to power the motor with 24V, configure CAN termination resistors, and set the motor ID; by default they come with ID 0, which is invalid.

# Build
TODO I'm not sure why, but building stalls at the `cargo expand` step if it is not in `--release` mode.
```
cargo install cargo-expand
cargo build --release
```

# Examples
By default the examples connect to a motor on `can0` with ID `1`. They command the motor in `voltage` mode and display `velocity` values.
These choices are set as constants at the top of the examples and may need to be changed to work with your system.

### [Rust example](gm6020_can/examples/gm6020_can_test.rs)
```
cd gm6020_can
cargo run --release --example gm6020_can_test
```

### [C++ example](examples/gm6020_can_test_cpp.rs)
```
cargo run --release --example gm6020_can_test_cpp
```



<img src="gm6020_can_test_cpp.gif" alt="gm6020_can_test_cpp"  loop=infinite>
