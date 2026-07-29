#include <memory>
#include <rclcpp/rclcpp.hpp>
#include "issem_ros2_bridge/bridge_node.hpp"

int main(int argc, char ** argv) {
    rclcpp::init(argc, argv);
    auto node = std::make_shared<issem::BridgeNode>();
    rclcpp::spin(node);
    rclcpp::shutdown();
    return 0;
}