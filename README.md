# `gm6020_can`
This Rust library controls a DJI GM6020 motor over a Linux SocketCAN interface. Why Rust? I wanted to try something new and Rust seems like a good skill to have in 2024. Feel free to give feedback on my code by submitting an issue or PR.

# `gm6020_can_cpp`
This library provides a C/C++ wrapper over `gm6020_can`. A static library and dynamic library is created in the target directory, and header files are automatically generated
in the include directory. A neat way to include this in your C++ program is to use Corrosion, which will automatically build the Rust crate and create a CMake target to link against.

Note, the library which does the header generation does [not yet handle fully-qualified C function names](https://github.com/mozilla/cbindgen/issues/380).
Since this library exposes common function names like 'init', I recommend you manually edit the header and prefix them all with 'gm6020_can'.
TODO call a system command to do this in build.rs

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

### [C++ example](examples/gm6020_can_test_cpp.rs)
```
cargo run --release --example gm6020_can_test_cpp
```

### [Rust example](gm6020_can/examples/gm6020_can_test.rs)
```
cd gm6020_can
cargo run --release --example gm6020_can_test
```

TODO gif of example running