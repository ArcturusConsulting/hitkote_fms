import os
from launch import LaunchDescription
from launch.actions import IncludeLaunchDescription, TimerAction, ExecuteProcess
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch.substitutions import PathJoinSubstitution
from launch_ros.substitutions import FindPackageShare

# Middleware protection to ensure robot2 shares the same DDS domain parameters
os.environ['RMW_IMPLEMENTATION'] = 'rmw_cyclonedds_cpp'
os.environ['CYCLONEDDS_URI'] = '<CycloneDDS><Domain><Discovery><MaxAutoParticipantIndex>500</MaxAutoParticipantIndex></Discovery></Domain></CycloneDDS>'

def create_initial_pose(namespace, x, y):
    """
    Polls until AMCL is in the 'active' state, then streams initial pose packets
    with transient_local durability to guarantee localized map anchors.
    """
    bash_script = (
        f'until ros2 lifecycle get /{namespace}/amcl 2>/dev/null | grep -qw "active"; do sleep 0.5; done; '
        f'ros2 topic pub --times 10 -r 2 --qos-durability transient_local /{namespace}/initialpose '
        f'geometry_msgs/msg/PoseWithCovarianceStamped '
        f'\'{{header: {{frame_id: "map"}}, pose: {{pose: {{position: {{x: {x}, y: {y}, z: 0.0}}, orientation: {{w: 1.0}}}}}}}}\''
    )
    return ExecuteProcess(
        cmd=['bash', '-c', bash_script],
        output='screen'
    )

def generate_launch_description():
    pkg_tb4_gz = FindPackageShare('turtlebot4_gz_bringup')

    # 1. Spawn Robot 2 into the existing Gazebo simulation world at x=2.0, y=1.0
    spawn_robot2 = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            PathJoinSubstitution([pkg_tb4_gz, 'launch', 'turtlebot4_spawn.launch.py'])
        ),
        launch_arguments={
            'namespace': 'robot2',
            'robot_name': 'robot2',
            'use_sim_time': 'true',
            'x': '2.0',
            'y': '1.0',
            'z': '0.0',
            'yaw': '0.0',
            'nav2': 'true',
            'localization': 'true',
            'slam': 'false',
            'rviz': 'false',
            'turtlebot4_camera': 'false',
        }.items()
    )

    # 2. Smart Initial Pose Publisher for Robot 2
    # Waits until /robot2/amcl reports 'active', then anchors initial pose at (2.0, 1.0)
    r2_pose = TimerAction(
        period=3.0,
        actions=[create_initial_pose('robot2', '2.0', '1.0')]
    )

    return LaunchDescription([
        spawn_robot2,
        r2_pose,
    ])