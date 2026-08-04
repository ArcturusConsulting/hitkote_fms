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

/// @brief Bridge node interfacing ISSEM Core FMS (Zenoh/VDA 5050) with the local AMR ROS 2 stack (Nav2/TF2).
class BridgeNode : public rclcpp::Node {
public:
    using NavigateToPose = nav2_msgs::action::NavigateToPose;
    using GoalHandleNav = rclcpp_action::ClientGoalHandle<NavigateToPose>;

    /// @brief Constructs the ROS 2 Bridge Node, initializing parameters, Nav2 action client, and TF2 listeners.
    /// @param options ROS 2 node options for composition and parameter overrides.
    explicit BridgeNode(const rclcpp::NodeOptions & options = rclcpp::NodeOptions());

    /// @brief Destructor ensuring clean shutdown of active Zenoh sessions and subscriptions.
    ~BridgeNode() override;

private:
    /// @brief Opens the Zenoh session, declares keyexpressions, and registers subscribers and publishers.
    void init_zenoh();

    /// @brief Safely undeclares Zenoh entities and closes the active Zenoh session.
    void cleanup_zenoh();

    /// @brief Static C-style callback triggered when a VDA 5050 order message arrives over Zenoh.
    /// @param sample The incoming Zenoh message sample.
    /// @param arg Raw pointer to the `BridgeNode` instance context (`void*`).
    static void on_order_received(z_loaned_sample_t * sample, void * arg);

    /// @brief Static C-style callback triggered when a VDA 5050 instantAction message arrives over Zenoh.
    /// @param sample The incoming Zenoh message sample.
    /// @param arg Raw pointer to the `BridgeNode` instance context (`void*`).
    static void on_instant_action_received(z_loaned_sample_t * sample, void * arg);

    /// @brief Converts target coordinates into a PoseStamped goal and dispatches it to Nav2 asynchronously.
    /// @param x Target X position in map frame (meters).
    /// @param y Target Y position in map frame (meters).
    /// @param yaw Target planar heading angle (radians).
    /// @param node_id VDA 5050 topological target node ID.
    void dispatch_nav2_goal(double x, double y, double yaw, const std::string & node_id);

    /// @brief Asynchronously requests cancellation of the currently active Nav2 goal.
    void cancel_nav2_goal();

    /// @brief Callback invoked when the Nav2 action server accepts or rejects a dispatched goal.
    /// @param goal_handle Shared pointer to the accepted goal handle, or nullptr if rejected.
    void goal_response_callback(const GoalHandleNav::SharedPtr & goal_handle);

    /// @brief Callback receiving periodic execution feedback from the active Nav2 action.
    /// @param goal_handle Goal handle associated with the feedback.
    /// @param feedback Pointer to the Nav2 navigation feedback message.
    void feedback_callback(
        GoalHandleNav::SharedPtr goal_handle,
        const std::shared_ptr<const NavigateToPose::Feedback> feedback);

    /// @brief Callback invoked when Nav2 finishes, aborts, or cancels a goal execution.
    /// @param result Wrapped result structure containing status code and final response.
    void result_callback(const GoalHandleNav::WrappedResult & result);

    /// @brief Timer-driven callback (5 Hz) that publishes real-time robot pose and status to Zenoh in VDA 5050 format.
    void publish_telemetry();

    /// @brief Publishes auxiliary state information for debug and GUI visualization over Zenoh.
    void publish_visualization();

    /// @brief Generates an ISO 8601 formatted UTC timestamp string (e.g., "YYYY-MM-DDTHH:MM:SS.mmmZ") using node time.
    /// @return Formatted timestamp string supporting both system and ROS simulation time.
    std::string get_iso_utc_timestamp();

    // --- Node Parameters ---
    std::string agv_id_;
    std::string zenoh_locator_;

    // --- ROS 2 Nav2 & TF Infrastructure ---
    rclcpp_action::Client<NavigateToPose>::SharedPtr nav2_client_;
    std::shared_ptr<tf2_ros::Buffer> tf_buffer_;
    std::shared_ptr<tf2_ros::TransformListener> tf_listener_;
    rclcpp::TimerBase::SharedPtr telemetry_timer_;

    // --- Navigation Tracking State ---
    GoalHandleNav::SharedPtr current_goal_handle_;
    std::string current_order_id_;
    uint32_t current_action_id_{0};
    std::string nav_state_{"NAVIGATING"};

    // --- Zenoh C Handles & Session State ---
    bool zenoh_connected_{false};
    z_owned_session_t zenoh_session_{};
    z_owned_subscriber_t order_sub_{};
    z_owned_subscriber_t action_sub_{};
    z_owned_publisher_t state_pub_{};
    z_owned_publisher_t vis_pub_{};

    // --- Protocol Tracking ---
    uint32_t header_id_{0};
};

}  // namespace issem

#endif  // ISSEM_ROS2_BRIDGE__BRIDGE_NODE_HPP_