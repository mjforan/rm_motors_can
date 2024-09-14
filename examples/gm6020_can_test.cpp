#include <iostream>

#include "../include/gm6020_can.h"
#include <stdio.h>
#include <thread>
#include <chrono>
#include <iostream>
#include <iomanip>
#include <cmath>
#include <atomic>
#include <memory>
#include <signal.h>

//////
// Basic C++ example showing how to use gm6020_can library. Corresponds to gm6020_can/examples/gm6020_can_test.rs
//////
// cargo run --release --example gm6020_can_test

const unsigned int INC = 10;                                         // Time (ms) between commands in the for loops
const int MAX = gm6020_can::V_MAX * 10;                              // Need the 10x multiplier so we can easily increment in for loops (can't increment floats).
const int ID = 1;                                                    // Motor ID [1,7]
const gm6020_can::FbField FB_FIELD = gm6020_can::FbField::Velocity;  // The feedback value to visualize

// To match the Rust example we would use something like this:
//   std::shared_ptr<std::atomic_bool> shared_final = std::make_shared<std::atomic_bool>(std::atomic_bool(false));
// but in C++ it is not possible to pass additional variables to the signal handler. So we must use global variables.
std::atomic_bool shared_stop = false;
std::atomic_bool shared_final = false;
gm6020_can::Gm6020Can* gmc = nullptr;
void print_output(gm6020_can::Gm6020Can* gm6020_can);

extern "C" int gm6020_can_test_cpp() {
    // Open SocketCAN device
    gmc = gm6020_can::init("can0");
    if (gmc == nullptr){
        std::cerr<<"Unable to open specified SocketCAN device"<<std::endl;
        return -1;
    }

    // TODO join all threads
    std::thread thread_out([](){
        while (! shared_stop.load()){
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
            print_output(gmc);
        }
    });

    signal (SIGINT, [](int){
        shared_stop.store(true);
        if (gmc != nullptr)
            gm6020_can::cleanup(gmc, 5);
        shared_final.store(true);
    });
    // Start another thread to collect feedback values
    std::thread thread_run([](){
        while (!shared_stop.load()) {
            gm6020_can::run_once(gmc);
            std::this_thread::sleep_for(std::chrono::milliseconds(INC));
        }
    });

    // Ramp up, ramp down, ramp up (negative), ramp down (negative)
    for (int voltage = 0; voltage <= MAX; voltage += 2) {
        if (shared_stop.load()) break; // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, voltage / 10.0);
        std::this_thread::sleep_for (std::chrono::milliseconds(INC));
    }
    for (int voltage = MAX; voltage > 0; voltage -= 2) {
        if (shared_stop.load()) break; // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, voltage / 10.0);
        std::this_thread::sleep_for (std::chrono::milliseconds(INC));
    }
    for (int voltage = 0; voltage >= -1*MAX; voltage -=2) {
        if (shared_stop.load()) break; // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, voltage / 10.0);
        std::this_thread::sleep_for (std::chrono::milliseconds(INC));
    }
    for (int voltage = -1*MAX+1; voltage < 1; voltage += 2) {
        if (shared_stop.load()) break; // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, voltage / 10.0);
        std::this_thread::sleep_for (std::chrono::milliseconds(INC));
    }

    // Send constant voltage command and read out position feedback
    gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, 2.0);
    while (! shared_final.load()){
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    return 0;
}

// Print out a simple bar chart of feedback values
void print_output(gm6020_can::Gm6020Can* gm6020_can) {
    double val = gm6020_can::get_state(gm6020_can, ID, FB_FIELD);
    std::cout<<std::fixed<<std::setprecision(3)<<val<<"\t";
    unsigned int n = 0;
    switch (FB_FIELD) {
        case gm6020_can::FbField::Position:
        n = val*5.0;
        break;
        case gm6020_can::FbField::Velocity:
        n = abs(val);
        break;
        case gm6020_can::FbField::Current:
        n = abs(val)*10.0;
        break;
        case gm6020_can::FbField::Temperature:
        n = val;
        break;
    }
    std::cout<<std::string(n, '#')<<std::endl;
}
