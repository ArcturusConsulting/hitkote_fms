cmake_minimum_required(VERSION 3.14)
project(hitkote_ros2_bridge)

if(CMAKE_COMPILER_IS_GNUCXX OR CMAKE_CXX_COMPILER_ID MATCHES "Clang")
  add_compile_options(-Wall -Wextra -Wpedantic)
endif()

if(NOT CMAKE_CXX_STANDARD)
  set(CMAKE_CXX_STANDARD 17)
endif()

# Configure RPATH so installed binaries look in ../lib for shared libraries
set(CMAKE_INSTALL_RPATH "$ORIGIN;$ORIGIN/..")
set(CMAKE_BUILD_WITH_INSTALL_RPATH TRUE)

# 1. Find ROS 2 & System Dependencies
find_package(ament_cmake REQUIRED)
find_package(rclcpp REQUIRED)
find_package(rclcpp_action REQUIRED)
find_package(rclcpp_components REQUIRED)
find_package(nav2_msgs REQUIRED)
find_package(geometry_msgs REQUIRED)
find_package(tf2_ros REQUIRED)
find_package(tf2_geometry_msgs REQUIRED)
find_package(nlohmann_json REQUIRED)

# 2. Automatically Fetch & Build zenoh-c if not installed on system
include(FetchContent)
find_package(zenohc QUIET)

if(NOT zenohc_FOUND)
  message(STATUS "zenohc not found locally. Fetching from GitHub...")
  FetchContent_Declare(
    zenohc
    GIT_REPOSITORY https://github.com/eclipse-zenoh/zenoh-c.git
    GIT_TAG        main
  )
  FetchContent_MakeAvailable(zenohc)
endif()

# 3. Component Shared Library (Plugin)
add_library(bridge_node_component SHARED src/bridge_node.cpp)

target_include_directories(bridge_node_component PUBLIC
  $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
  $<INSTALL_INTERFACE:include>
)

# Link Zenoh & JSON
if(TARGET zenohc::lib)
  target_link_libraries(bridge_node_component zenohc::lib)
elseif(TARGET zenohc::shared)
  target_link_libraries(bridge_node_component zenohc::shared)
else()
  target_link_libraries(bridge_node_component zenohc)
endif()

target_link_libraries(bridge_node_component nlohmann_json::nlohmann_json)

# ROS 2 Target Dependencies
ament_target_dependencies(bridge_node_component
  rclcpp
  rclcpp_action
  rclcpp_components
  nav2_msgs
  geometry_msgs
  tf2_ros
  tf2_geometry_msgs
)

# Register as ROS 2 Component Container plugin
rclcpp_components_register_nodes(bridge_node_component "hitkote::BridgeNode")

# 4. Standalone Executable Node
add_executable(bridge_node src/bridge_node_main.cpp)

target_include_directories(bridge_node PUBLIC
  $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
  $<INSTALL_INTERFACE:include>
)

target_link_libraries(bridge_node bridge_node_component)

ament_target_dependencies(bridge_node
  rclcpp
)

# 5. Installation Directives
# Use generator expressions to copy the fetched library file directly
if(TARGET zenohc::shared)
  install(FILES $<TARGET_FILE:zenohc::shared> DESTINATION lib)
elseif(TARGET zenohc::lib)
  install(FILES $<TARGET_FILE:zenohc::lib> DESTINATION lib)
elseif(TARGET zenohc)
  install(TARGETS zenohc DESTINATION lib)
endif()

install(TARGETS bridge_node_component bridge_node
  ARCHIVE DESTINATION lib
  LIBRARY DESTINATION lib
  RUNTIME DESTINATION lib/${PROJECT_NAME}
)

install(DIRECTORY include/
  DESTINATION include
)

ament_package()