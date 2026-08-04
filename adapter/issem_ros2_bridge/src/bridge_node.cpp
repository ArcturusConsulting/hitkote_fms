#include "issem_ros2_bridge/bridge_node.hpp"

#include <cmath>
#include <nlohmann/json.hpp>
#include <tf2_geometry_msgs/tf2_geometry_msgs.hpp>

using json = nlohmann::json;

namespace issem {

BridgeNode::BridgeNode(const rclcpp::NodeOptions & options)
: Node("issem_ros2_bridge", options) {
    // Declare parameters from bridge_params.yaml
    this->declare_parameter<std::string>("agv_id", "AMR-01");
    this->declare_parameter<std::string>("manufacturer", "Mir");
    this->declare_parameter<std::string>("zenoh_locator", "tcp/127.0.0.1:7447");

    agv_id_ = this->get_parameter("agv_id").as_string();
    zenoh_locator_ = this->get_parameter("zenoh_locator").as_string();

    nav2_client_ = rclcpp_action::create_client<NavigateToPose>(this, "navigate_to_pose");

    tf_buffer_ = std::make_shared<tf2_ros::Buffer>(this->get_clock());
    tf_listener_ = std::make_shared<tf2_ros::TransformListener>(*tf_buffer_);

    init_zenoh();

    telemetry_timer_ = this->create_wall_timer(
        std::chrono::milliseconds(200),
        std::bind(&BridgeNode::publish_telemetry, this));

    RCLCPP_INFO(this->get_logger(), "ISSEM ROS2 Bridge initialized for AGV: %s", agv_id_.c_str());
}

BridgeNode::~BridgeNode() {
    cleanup_zenoh();
}

void BridgeNode::init_zenoh() {
    z_owned_config_t config;
    z_config_default(&config);

    if (!zenoh_locator_.empty()) {
        std::string json5_locator = "[\"" + zenoh_locator_ + "\"]";
        zc_config_insert_json5(z_loan_mut(config), Z_CONFIG_CONNECT_KEY, json5_locator.c_str());
    }

    if (z_open(&zenoh_session_, z_move(config), NULL) < 0) {
        RCLCPP_ERROR(this->get_logger(), "Failed to open Zenoh session!");
        zenoh_connected_ = false;
        return;
    }

    // Fetch parameters dynamically for topic construction
    std::string manufacturer = this->get_parameter("manufacturer").as_string();

    // Must match the exact topic structure the ISSEM Core expects
    std::string order_topic = "uagv/v2/" + manufacturer + "/" + agv_id_ + "/order";
    std::string action_topic = "uagv/v2/" + manufacturer + "/" + agv_id_ + "/instantActions";
    std::string state_topic = "issem/v3/" + manufacturer + "/" + agv_id_ + "/state";
    std::string vis_topic = "issem/v3/" + manufacturer + "/" + agv_id_ + "/visualization";

    // Setup Subscribers: z_declare_subscriber(session_loan, &sub_handle, keyexpr_loan, closure_move, options)
    z_owned_closure_sample_t order_closure;
    z_closure_sample(&order_closure, on_order_received, NULL, this);
    z_view_keyexpr_t order_ke;
    z_view_keyexpr_from_str(&order_ke, order_topic.c_str());
    z_declare_subscriber(z_loan(zenoh_session_), &order_sub_, z_loan(order_ke), z_move(order_closure), NULL);

    z_owned_closure_sample_t action_closure;
    z_closure_sample(&action_closure, on_instant_action_received, NULL, this);
    z_view_keyexpr_t action_ke;
    z_view_keyexpr_from_str(&action_ke, action_topic.c_str());
    z_declare_subscriber(z_loan(zenoh_session_), &action_sub_, z_loan(action_ke), z_move(action_closure), NULL);

    // Setup Publishers: z_declare_publisher(session_loan, &pub_handle, keyexpr_loan, options)
    z_view_keyexpr_t state_ke;
    z_view_keyexpr_from_str(&state_ke, state_topic.c_str());
    z_declare_publisher(z_loan(zenoh_session_), &state_pub_, z_loan(state_ke), NULL);

    z_view_keyexpr_t vis_ke;
    z_view_keyexpr_from_str(&vis_ke, vis_topic.c_str());
    z_declare_publisher(z_loan(zenoh_session_), &vis_pub_, z_loan(vis_ke), NULL);

    zenoh_connected_ = true;
    RCLCPP_INFO(this->get_logger(), "Zenoh session connected. Subscribed & Publishing for %s/%s.", manufacturer.c_str(), agv_id_.c_str());
}

void BridgeNode::cleanup_zenoh() {
    if (zenoh_connected_) {
        zenoh_connected_ = false;
        z_undeclare_subscriber(z_move(order_sub_));
        z_undeclare_subscriber(z_move(action_sub_));
        z_undeclare_publisher(z_move(state_pub_));
        z_undeclare_publisher(z_move(vis_pub_));
        z_close(z_loan_mut(zenoh_session_), NULL);
        z_drop(z_move(zenoh_session_));
    }
}

void BridgeNode::on_order_received(z_loaned_sample_t * sample, void * arg) {
    auto * node = static_cast<BridgeNode *>(arg);
    z_owned_string_t payload_str;
    z_bytes_to_string(z_sample_payload(sample), &payload_str);
    std::string payload_json(z_string_data(z_loan(payload_str)), z_string_len(z_loan(payload_str)));
    z_drop(z_move(payload_str));

    RCLCPP_INFO(node->get_logger(), "====== ZENOH MESSAGE RECEIVED ======");
    RCLCPP_INFO(node->get_logger(), "Raw Payload: %s", payload_json.c_str());

    try {
        auto j = json::parse(payload_json);
        node->current_order_id_ = j.value("orderId", "");
        if (j.contains("nodes") && !j["nodes"].empty()) {
            auto target_node = j["nodes"].back();
            if (target_node.contains("nodePosition")) {
                auto pos = target_node["nodePosition"];
                double x = pos.value("x", 0.0);
                double y = pos.value("y", 0.0);
                double yaw = pos.value("theta", 0.0);
                std::string node_id = target_node.value("nodeId", "");
                node->dispatch_nav2_goal(x, y, yaw, node_id);
            }
        }
    } catch (const std::exception & e) {
        RCLCPP_ERROR(node->get_logger(), "Failed to parse order JSON: %s", e.what());
    }
}

void BridgeNode::on_instant_action_received(z_loaned_sample_t * sample, void * arg) {
    auto * node = static_cast<BridgeNode *>(arg);
    z_owned_string_t payload_str;
    z_bytes_to_string(z_sample_payload(sample), &payload_str);
    std::string payload_json(z_string_data(z_loan(payload_str)), z_string_len(z_loan(payload_str)));
    z_drop(z_move(payload_str));

    try {
        auto j = json::parse(payload_json);
        if (j.contains("actions")) {
            for (const auto & action : j["actions"]) {
                std::string action_type = action.value("actionType", "");
                if (action_type == "cancelOrder" || action_type == "stop") {
                    node->cancel_nav2_goal();
                }
            }
        }
    } catch (const std::exception & e) {
        RCLCPP_ERROR(node->get_logger(), "Failed to parse instant action JSON: %s", e.what());
    }
}

void BridgeNode::dispatch_nav2_goal(double x, double y, double yaw, const std::string & node_id) {
    (void)node_id;
    if (!nav2_client_->wait_for_action_server(std::chrono::seconds(2))) {
        RCLCPP_ERROR(this->get_logger(), "Nav2 action server not available!");
        return;
    }

    auto goal_msg = NavigateToPose::Goal();
    goal_msg.pose.header.frame_id = "map";
    goal_msg.pose.header.stamp = this->now();
    goal_msg.pose.pose.position.x = x;
    goal_msg.pose.pose.position.y = y;

    goal_msg.pose.pose.orientation.z = std::sin(yaw / 2.0);
    goal_msg.pose.pose.orientation.w = std::cos(yaw / 2.0);

    auto send_goal_options = rclcpp_action::Client<NavigateToPose>::SendGoalOptions();
    send_goal_options.goal_response_callback = std::bind(&BridgeNode::goal_response_callback, this, std::placeholders::_1);
    send_goal_options.feedback_callback = std::bind(&BridgeNode::feedback_callback, this, std::placeholders::_1, std::placeholders::_2);
    send_goal_options.result_callback = std::bind(&BridgeNode::result_callback, this, std::placeholders::_1);

    nav2_client_->async_send_goal(goal_msg, send_goal_options);
    nav_state_ = "NAVIGATING";
}

void BridgeNode::cancel_nav2_goal() {
    if (current_goal_handle_) {
        nav2_client_->async_cancel_goal(current_goal_handle_);
        nav_state_ = "CANCELLED";
    }
}

void BridgeNode::goal_response_callback(const GoalHandleNav::SharedPtr & goal_handle) {
    if (!goal_handle) {
        RCLCPP_ERROR(this->get_logger(), "Goal was rejected by server");
        nav_state_ = "REJECTED";
    } else {
        current_goal_handle_ = goal_handle;
        RCLCPP_INFO(this->get_logger(), "Goal accepted by server, waiting for result");
    }
}

void BridgeNode::feedback_callback(
    GoalHandleNav::SharedPtr goal_handle,
    const std::shared_ptr<const NavigateToPose::Feedback> feedback) {
    (void)goal_handle;
    (void)feedback;
}

void BridgeNode::result_callback(const GoalHandleNav::WrappedResult & result) {
    switch (result.code) {
        case rclcpp_action::ResultCode::SUCCEEDED:
            nav_state_ = "FINISHED";
            RCLCPP_INFO(this->get_logger(), "Goal succeeded!");
            break;
        case rclcpp_action::ResultCode::ABORTED:
            nav_state_ = "FAILED";
            RCLCPP_ERROR(this->get_logger(), "Goal was aborted");
            break;
        case rclcpp_action::ResultCode::CANCELED:
            nav_state_ = "CANCELLED";
            RCLCPP_WARN(this->get_logger(), "Goal was canceled");
            break;
        default:
            nav_state_ = "FAILED";
            break;
    }
}

void BridgeNode::publish_telemetry() {
    if (!zenoh_connected_) return;

    double x = 0.0, y = 0.0, yaw = 0.0;
    try {
        auto tf = tf_buffer_->lookupTransform("map", "base_link", tf2::TimePointZero);
        x = tf.transform.translation.x;
        y = tf.transform.translation.y;

        double qz = tf.transform.rotation.z;
        double qw = tf.transform.rotation.w;
        yaw = 2.0 * std::atan2(qz, qw);
    } catch (const tf2::TransformException & ex) {
        // TF lookup might fail initially before robot spawns
    }

    std::string manufacturer = this->get_parameter("manufacturer").as_string();

    json state_json = {
        {"headerId", ++header_id_},
        {"timestamp", get_iso_utc_timestamp()},
        {"version", "3.0.0"},
        {"manufacturer", manufacturer},
        {"serialNumber", agv_id_},
        {"orderId", current_order_id_},
        {"lastNodeSequenceId", 0},
        {"driving", (nav_state_ == "NAVIGATING")},
        {"agvPosition", {
            {"x", x},
            {"y", y},
            {"theta", yaw},
            {"mapId", "map"},
            {"positionInitialized", true}
        }},
        {"operatingMode", "AUTOMATIC"},
        {"safetyStatus", {
            {"eStop", "NONE"},
            {"fieldViolation", false}
        }}
    };

    std::string state_str = state_json.dump();
    z_publisher_put_options_t options;
    z_publisher_put_options_default(&options);
    z_owned_bytes_t payload;
    z_bytes_copy_from_str(&payload, state_str.c_str());
    z_publisher_put(z_loan(state_pub_), z_move(payload), &options);
}

void BridgeNode::publish_visualization() {
    if (!zenoh_connected_) return;

    json vis_json = {
        {"agvId", agv_id_},
        {"state", nav_state_}
    };

    std::string vis_str = vis_json.dump();
    z_publisher_put_options_t options;
    z_publisher_put_options_default(&options);
    z_owned_bytes_t payload;
    z_bytes_copy_from_str(&payload, vis_str.c_str());
    z_publisher_put(z_loan(vis_pub_), z_move(payload), &options);
}

std::string BridgeNode::get_iso_utc_timestamp() {
    // Get time from the ROS 2 node clock (supports simulation time seamlessly)
    rclcpp::Time now = this->now();
    
    // Extract seconds and milliseconds
    int64_t nanos = now.nanoseconds();
    std::time_t seconds = static_cast<std::time_t>(nanos / 1'000'000'000);
    uint32_t millis = static_cast<uint32_t>((nanos % 1'000'000'000) / 1'000'000);

    // Convert to UTC broken-down time
    std::tm bt{};
    gmtime_r(&seconds, &bt);

    // Format ISO 8601 timestamp: YYYY-MM-DDTHH:MM:SS.mmmZ
    std::ostringstream oss;
    oss << std::put_time(&bt, "%Y-%m-%dT%H:%M:%S")
        << '.' << std::setfill('0') << std::setw(3) << millis
        << 'Z';

    return oss.str();
}

}  // namespace issem

#include "rclcpp_components/register_node_macro.hpp"
RCLCPP_COMPONENTS_REGISTER_NODE(issem::BridgeNode)