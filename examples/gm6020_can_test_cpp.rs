// TODO ideally we would call test_cpp() here directly, but there is a bug in the cc crate where we have to do it from the library
fn main() {
    unsafe {gm6020_can_cpp::cpp_example()};
}