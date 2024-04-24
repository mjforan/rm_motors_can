```
cargo build --release
g++ -o main main.cpp -I include -L target/release -l gm6020_can
./main
```
