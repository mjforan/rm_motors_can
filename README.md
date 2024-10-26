# `rm_motors_can`
This Rust library controls a DJI RoboMaster motors over a Linux SocketCAN interface.

<img src="rm_motors_can_test_cpp.gif" alt="rm_motors_can_test_cpp"  loop=infinite>

TODO update table with specs for other motors (currently just GM6020)

<table>
<tr><td>

| Control Mode | Max  | Units |
|----------|------|-------|
| Voltage  | 24   | V     |
| Current  | 1.62 | A     |
| Velocity | 33.5 | rad/s |
| Torque   | 1.2  | N*m   |

</td><td></td><td></td><td>

| Feedback Field | Units |
|-------------|-------|
| Position    | rad   |
| Velocity    | rad/s |
| Current     | A     |
| Temperature | °C    |

</td></tr></table>

Note that "current" is actually "torque current", which is the portion of current in-phase with the voltage, i.e. how much current is generating useful torque. So while you command 1.62A, keep in mind the motor could be drawing over 3A.

The velocity and torque commands are simply voltage and current commands scaled by constants given in the datasheet. These may not be accurate across the full range of running conditions. For better accuracy or position control your controller must utilize feedback from `get_state` - or consider using the PWM interface.

Switching from Voltage/Velocity to Current/Torque modes requires changing parameters in RoboMaster Assistant.

Temperature is only reported in whole-number precision.


# `rm_motors_can_cpp`
This library provides a C/C++ wrapper over `rm_motors_can`. Static and dynamic libraries are created in the target directory and header files are generated in the include directory. A neat way to include this in your C++ program is to use Corrosion, which will automatically build the Rust crate and create a CMake target to link against.

Unfortunately the C header does not contain "fully-qualified" names. Ideally each name would be prefixed with `rm_motors_can_` to avoid conflict of common names like `init`. There is some ongoing work in the `cbindgen` tool to address this. If it is an issue for your project, change the function names in Rust and uncomment the `[export]` block in [`cbindgen_c.toml`](cbindgen_c.toml) to prefix all other items.


# [`rm_motors_ros`](https://github.com/mjforan/rm_motors_ros/)
ROS 2 wrapper which enables advanced control interfaces such as `ros2_control` and `MoveIt`. Talk about layers of abstraction! This repo has an example hardware setup and `CMakeLists.txt`.


# Hardware
The motor should be accessible via a SocketCAN interface. This can be accomplished with a USB CAN adapter, Raspberry Pi HAT, or built-in CAN interface like on an NVIDIA Orin. Don't forget to power the motor with 24V, configure CAN termination resistors, and set the motor ID; from the factory the ID is 0, which is invalid for GM6020.


# Build
TODO building stalls at the `cargo expand` step when not in `--release` mode.
```
cargo install cargo-expand
cargo build --release
```


# Examples
By default the examples connect to a motor on `can0` with ID `1`. They command the motor in `voltage` mode and display `velocity` values.
These choices are set as constants at the top of the examples and may need to be changed to work with your system.

### [Rust example](rm_motors_can/examples/rm_motors_can_test.rs)
```
cd rm_motors_can
cargo run --release --example rm_motors_can_test
```

### [C++ example](examples/rm_motors_can_test_cpp.rs)
```
cargo run --release --example rm_motors_can_test_cpp
```


# RoboMaster Assistant
When the datasheet says "Use black for GND, grey for TX, and white for PWM/RX", it means white is the RX pin of the motor and should be connected to the TX pin of your serial adapter. The adapter should be set for 5V logic levels. Only start RM Assistant AFTER the motor has booted up, otherwise it will not connect. You don't have to click anything on the blue waiting screen; it will automatically detect the motor after a few seconds. Click to open the motor and in the bottom left there is a language selector. After changing parameters you must click the "Settings" button in the bottom right to apply them. The "enable current ring" parameter switches between Voltage/Velocity and Current/Torque control modes.
