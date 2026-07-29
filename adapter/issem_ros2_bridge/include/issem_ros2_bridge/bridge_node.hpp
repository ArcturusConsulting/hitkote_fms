#ifndef ISSEM_ROS2_BRIDGE__BRIDGE_NODE_HPP_
#define ISSEM_ROS2_BRIDGE__BRIDGE_NODE_HPP_

#include <memory>
#include <string>

#include <rclcpp/rclcpp.hpp>
#include <rclcpp_action/rclcpp_action.hpp>
#include <geometry_msgs/msg/pose_stamped.hpp>
#include <nav2_msgs/action/navigate_to_pose.hpp>
#include <tf2_ros/buffer.h>
#include <tf2_ros/transform_listener.h>

#include <zenoh.h>

namespace issem {

class BridgeNode : public rclcpp::Node {
public:
    using NavigateToPose = nav2_msgs::action::NavigateToPose;
    using GoalHandleNav = rclcpp_action::ClientGoalHandle<NavigateToPose>;

    explicit BridgeNode(const rclcpp::NodeOptions & options = rclcpp::NodeOptions());
    ~BridgeNode() override;

private:
    void init_zenoh();
    void cleanup_zenoh();

    // Zenoh 1.0 Callback Signatures
    static void on_order_received(z_loaned_sample_t * sample, void * arg);
    static void on_instant_action_received(z_loaned_sample_t * sample, void * arg);

    void dispatch_nav2_goal(double x, double y, double yaw, const std::string & node_id);
    void cancel_nav2_goal();

    void goal_response_callback(const GoalHandleNav::SharedPtr & goal_handle);
    void feedback_callback(
        GoalHandleNav::SharedPtr goal_handle,
        const std::shared_ptr<const NavigateToPose::Feedback> feedback);
    void result_callback(const GoalHandleNav::WrappedResult & result);

    void publish_telemetry();
    void publish_visualization();

    // Node Parameters
    std::string agv_id_;
    std::string zenoh_locator_;

    // ROS 2 Nav2 & TF
    rclcpp_action::Client<NavigateToPose>::SharedPtr nav2_client_;
    std::shared_ptr<tf2_ros::Buffer> tf_buffer_;
    std::shared_ptr<tf2_ros::TransformListener> tf_listener_;
    rclcpp::TimerBase::SharedPtr telemetry_timer_;

    // Current Navigation State
    GoalHandleNav::SharedPtr current_goal_handle_;
    std::string current_order_id_;
    uint32_t current_action_id_{0};
    std::string nav_state_{"NAVIGATING"};

    // Zenoh C Handles & State
    bool zenoh_connected_{false};
    z_owned_session_t zenoh_session_{};
    z_owned_subscriber_t order_sub_{};
    z_owned_subscriber_t action_sub_{};
    z_owned_publisher_t state_pub_{};
    z_owned_publisher_t vis_pub_{};
};

}  // namespace issem

#endif  // ISSEM_ROS2_BRIDGE__BRIDGE_NODE_HPP_