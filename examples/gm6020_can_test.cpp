#include <iostream>

#include "../include/gm6020_can.hpp"
#include <stdio.h>
#include <thread>
#include <chrono>
#include <iostream>
#include <iomanip>
#include <cmath>
#include <atomic>
#include <memory>
#include <signal.h>
#include <vector>

//////
// Basic C++ example showing how to use gm6020_can library. Corresponds to gm6020_can/examples/gm6020_can_test.rs
//////
// cargo run --example gm6020_can_test_cpp

const unsigned int INC = 10;                                         // Time (ms) between commands in the for loops
const int MAX = gm6020_can::V_MAX * 10;                              // Need the 10x multiplier so we can easily increment in for loops (can't increment floats).
const int ID = 1;                                                    // Motor ID [1,7]
const gm6020_can::FbField FB_FIELD = gm6020_can::FbField::Velocity;  // The feedback value to visualize
const char* CAN_INTERFACE = "can0";                                  // SocketCAN interface to open

// To match the Rust example we would use something like this:
//   std::shared_ptr<std::atomic_bool> shared_final = std::make_shared<std::atomic_bool>(std::atomic_bool(false));
// but in C++ it is not possible to pass additional variables to the signal handler so we must use global variables.
std::atomic_bool shared_stop = false;
std::atomic_bool shared_final = false;
gm6020_can::Gm6020Can* gmc = nullptr;
void print_output(gm6020_can::Gm6020Can* gm6020_can);

extern "C" int gm6020_can_test_cpp() {
    // Open SocketCAN device
    gmc = gm6020_can::init(CAN_INTERFACE);
    if (gmc == nullptr){
        std::cerr<<"Unable to open specified SocketCAN device"<<std::endl;
        return -1;
    }
    std::vector<std::thread> threads;

    threads.emplace_back(std::thread([](){
        while (! shared_stop.load()){
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
            print_output(gmc);
        }
    }));

    // Set up a signal handler to clean up (not strictly necessary but good practice)
    signal (SIGINT, [](int){
        // stop the other threads
        shared_stop.store(true);
        // gently turn off the motors
        gm6020_can::cleanup(gmc, 5);
        // stop this thread
        shared_final.store(true);
    });

    // Start another thread to periodically collect feedbacks and write commands
    // It's better to run_once() after every set_cmd to minimize delay before writing,
    // but if this loop is fast enough it will not be noticeable. This approach has the advantage of
    // running consistently, which prevents the socket buffer from filling up in case e.g. the main thread is blocked.
    threads.emplace_back(std::thread([](){
        while (!shared_stop.load()) {
            gm6020_can::run_once(gmc);
            std::this_thread::sleep_for(std::chrono::milliseconds(INC));
        }
    }));

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

    // Send one last voltage command
    gm6020_can::set_cmd(gmc, ID, gm6020_can::CmdMode::Voltage, 2.0);
    // Wait for the ctl-c handler to finish cleaning up
    while (! shared_final.load()){
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    // Join all threads so it doesn't complain about unfinished business
    for (std::thread & thread : threads)
        thread.join();

    return 0;
}

// Print out a simple bar chart of feedback values
void print_output(gm6020_can::Gm6020Can* gm6020_can) {
    double val = gm6020_can::get_state(gm6020_can, ID, FB_FIELD);
     // Right justify, 7 wide, 2 decimal digits
    std::cout<<std::fixed<<std::setprecision(2)<<std::right<<std::setw(7)<<val<<std::left<<std::setw(0)<<"\t";
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
