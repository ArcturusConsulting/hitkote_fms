import os
from launch import LaunchDescription
from launch.actions import IncludeLaunchDescription, TimerAction, ExecuteProcess
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch.substitutions import LaunchConfiguration, PathJoinSubstitution
from launch_ros.substitutions import FindPackageShare

def generate_launch_description():
    use_sim_time = LaunchConfiguration('use_sim_time', default='true')
    nav2 = LaunchConfiguration('nav2', default='true')
    localization = LaunchConfiguration('localization', default='true')
    rviz = LaunchConfiguration('rviz', default='true')
    world = LaunchConfiguration('world', default='warehouse')

    tb4_gz_bringup_share = FindPackageShare('turtlebot4_gz_bringup')

    tb4_simulation_launch = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            PathJoinSubstitution([tb4_gz_bringup_share, 'launch', 'turtlebot4_gz.launch.py'])
        ),
        launch_arguments={
            'use_sim_time': use_sim_time,
            'nav2': nav2,
            'localization': localization,
            'rviz': rviz,
            'world': world,
            'x': '0.0',
            'y': '0.0',
            'z': '0.0',
            'yaw': '0.0',
        }.items()
    )

    # Latched publisher ensures AMCL gets the pose whenever it boots up
    latched_initial_pose = ExecuteProcess(
        cmd=[
            'ros2', 'topic', 'pub', '--once',
            '--qos-durability', 'transient_local',
            '/initialpose',
            'geometry_msgs/msg/PoseWithCovarianceStamped',
            '{'
            'header: {frame_id: "map"}, '
            'pose: {'
            '  pose: {position: {x: 0.0, y: 0.0, z: 0.0}, orientation: {w: 1.0}}, '
            '  covariance: [0.01, 0.0, 0.0, 0.0, 0.0, 0.0, '
            '               0.0, 0.01, 0.0, 0.0, 0.0, 0.0, '
            '               0.0, 0.0, 0.0, 0.0, 0.0, 0.0, '
            '               0.0, 0.0, 0.0, 0.0, 0.0, 0.0, '
            '               0.0, 0.0, 0.0, 0.0, 0.0, 0.0, '
            '               0.0, 0.0, 0.0, 0.0, 0.0, 0.01]'
            '}'
            '}'
        ],
        output='screen'
    )

    return LaunchDescription([
        tb4_simulation_launch,
        latched_initial_pose,
    ])