#include <iostream>
// TODO hardcoded paths
#include "/home/mjforan/phalanx/gm6020_ros/gm6020_hw/gm6020_can/include/gm6020_can_cpp.h"
extern "C" void gm6020_can_test_cpp() {
    std::cout<<"opening interface"<<std::endl;
    void* x = gm6020_can_init("can0");
    std::cout<<"done"<<std::endl;
} 