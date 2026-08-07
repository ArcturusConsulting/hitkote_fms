import os
from launch import LaunchDescription
from launch.actions import IncludeLaunchDescription, TimerAction, ExecuteProcess
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch.substitutions import PathJoinSubstitution
from launch_ros.substitutions import FindPackageShare

def create_initial_pose(namespace, x, y):
    """
    Waits dynamically until AMCL reaches the 'active' state,
    then publishes the initial pose with transient_local durability.
    """
    bash_script = (
        f'until ros2 lifecycle get /{namespace}/amcl 2>/dev/null | grep -qw "active"; do sleep 1; done; '
        f'ros2 topic pub --times 5 -r 1 --qos-durability transient_local /{namespace}/initialpose '
        f'geometry_msgs/msg/PoseWithCovarianceStamped '
        f'\'{{header: {{frame_id: "map"}}, pose: {{pose: {{position: {{x: {x}, y: {y}, z: 0.0}}, orientation: {{w: 1.0}}}}}}}}\''
    )
    return ExecuteProcess(
        cmd=['bash', '-c', bash_script],
        output='screen'
    )

def generate_launch_description():
    pkg_tb4_gz = FindPackageShare('turtlebot4_gz_bringup')

    # World + Robot 1 (Spawn at x=0.0, y=0.0)
    robot1 = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            PathJoinSubstitution([pkg_tb4_gz, 'launch', 'turtlebot4_gz.launch.py'])
        ),
        launch_arguments={
            'namespace': 'robot1',
            'robot_name': 'robot1',
            'use_sim_time': 'true',
            'x': '0.0', 
            'y': '0.0',
            'nav2': 'true',
            'localization': 'true',
            'slam': 'false',
            'rviz': 'false',
            'turtlebot4_camera': 'false', # Disables heavy 3D camera pointcloud bridge
        }.items()
    )

    # Smart Initial Pose Handler for Robot 1 (Waits dynamically for AMCL to be ACTIVE)
    r1_pose = TimerAction(period=3.0, actions=[create_initial_pose('robot1', '0.0', '0.0')])

    return LaunchDescription([
        robot1,
        r1_pose,
    ])