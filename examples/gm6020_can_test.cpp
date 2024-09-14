#include <iostream>

#include "../include/gm6020_can.h"
extern "C" void test_cpp() {
    std::cout<<"opening interface"<<std::endl;
    void* x = gm6020_can::init("can0");
    std::cout<<"done"<<std::endl;
}