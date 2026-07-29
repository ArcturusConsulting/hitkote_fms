import os
from launch import LaunchDescription
from launch.actions import DeclareLaunchArgument, IncludeLaunchDescription
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch.substitutions import LaunchConfiguration, PathJoinSubstitution
from launch_ros.substitutions import FindPackageShare

def generate_launch_description():
    # 1. Configurable Launch Arguments
    use_sim_time_arg = DeclareLaunchArgument(
        'use_sim_time',
        default_value='true',
        description='Use simulation clock'
    )
    
    nav2_arg = DeclareLaunchArgument(
        'nav2',
        default_value='true',
        description='Launch Nav2 stack'
    )
    
    localization_arg = DeclareLaunchArgument(
        'localization',
        default_value='true',
        description='Launch AMCL localization'
    )
    
    rviz_arg = DeclareLaunchArgument(
        'rviz',
        default_value='true',
        description='Launch RViz2 visualization'
    )

    world_arg = DeclareLaunchArgument(
        'world',
        default_value='warehouse',
        description='Gazebo world to load (e.g. warehouse, maze)'
    )

    # 2. Locate official turtlebot4_gz_bringup package
    tb4_gz_bringup_share = FindPackageShare('turtlebot4_gz_bringup')

    # 3. Include TurtleBot 4 Gazebo Bringup Launch File
    tb4_simulation_launch = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            PathJoinSubstitution([tb4_gz_bringup_share, 'launch', 'turtlebot4_gz.launch.py'])
        ),
        launch_arguments={
            'use_sim_time': LaunchConfiguration('use_sim_time'),
            'nav2': LaunchConfiguration('nav2'),
            'localization': LaunchConfiguration('localization'),
            'rviz': LaunchConfiguration('rviz'),
            'world': LaunchConfiguration('world'),
        }.items()
    )

    return LaunchDescription([
        use_sim_time_arg,
        nav2_arg,
        localization_arg,
        rviz_arg,
        world_arg,
        tb4_simulation_launch,
    ])