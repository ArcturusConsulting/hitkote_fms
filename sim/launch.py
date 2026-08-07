import os
from launch import LaunchDescription
from launch.actions import IncludeLaunchDescription, TimerAction, ExecuteProcess
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch.substitutions import PathJoinSubstitution
from launch_ros.actions import Node
from launch_ros.substitutions import FindPackageShare

# Middleware protection against DDS shared memory issues
os.environ['RMW_IMPLEMENTATION'] = 'rmw_cyclonedds_cpp'
os.environ['CYCLONEDDS_URI'] = '<CycloneDDS><Domain><Discovery><MaxAutoParticipantIndex>500</MaxAutoParticipantIndex></Discovery></Domain></CycloneDDS>'

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

    # 1. World + Robot 1 (Open aisle location at x=-1.0, y=-1.0)
    robot1 = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            PathJoinSubstitution([pkg_tb4_gz, 'launch', 'turtlebot4_gz.launch.py'])
        ),
        launch_arguments={
            'namespace': 'robot1',
            'robot_name': 'robot1',
            'x': '0.0', 
            'y': '0.0',
            'nav2': 'true',
            'localization': 'true',
            'slam': 'false',
            'rviz': 'false',
        }.items()
    )

    # 2. Spawn Robot 2 at t=8s
    robot2 = TimerAction(
        period=8.0,
        actions=[
            IncludeLaunchDescription(
                PythonLaunchDescriptionSource(
                    PathJoinSubstitution([pkg_tb4_gz, 'launch', 'turtlebot4_spawn.launch.py'])
                ),
                launch_arguments={
                    'namespace': 'robot2',
                    'robot_name': 'robot2',
                    'x': '2.0', 
                    'y': '1.0',
                    'nav2': 'true',
                    'localization': 'true',
                    'slam': 'false',
                    'rviz': 'false',
                }.items()
            )
        ]
    )

    # 3. Explicit Spawner for Robot 2 (Activates diffdrive_controller at t=14s)
    r2_diffdrive = TimerAction(
        period=14.0,
        actions=[
            Node(
                package='controller_manager',
                executable='spawner',
                arguments=[
                    'diffdrive_controller',
                    '-c', '/robot2/controller_manager',
                    '--controller-manager-timeout', '30'
                ],
                output='screen'
            )
        ]
    )

    # 4. Smart Initial Pose Handlers (Wait for AMCL to be ACTIVE before sending pose)
    r1_pose = TimerAction(period=5.0, actions=[create_initial_pose('robot1', '0.0', '0.0')])
    r2_pose = TimerAction(period=15.0, actions=[create_initial_pose('robot2', '2.0', '1.0')])

    return LaunchDescription([
        robot1,
        robot2,
        r2_diffdrive,
        r1_pose,
        r2_pose,
    ])