# loggen-rs

A Log Generator written in Rust

## Features

###  YAML based configuration

The configuration is in YAML. Every config option has its corresponding cli option.

### Stream Options

Stream logs to different outputs. Starting with stdout and file output.
In future releases streaming should be possible to an http endpoint or kafka broker

### Template System

Collect a set of log files in a folder and prepare streaming them with a jinja based template system.
Validation that templates produce valid log entries.
Examples and documentation for common log formats.
Default templates - Some basic templates included out-of-the-box

### Randomization

Generate realistic-looking but randomized log entries from the template directory

### Attack patterns generation

Create a set of log patterns that will mimic common attacks and their corresponding responses.
Actually parsing and applying Sigma rules to generate appropriate logs
Attacks are described in sigma and logen-rs creates the corresponding log data to trigger the rules. 

### Performance

Generate large volumes of logs efficiently.
This application is prepared to simulate high load situations
Progress reporting - For long-running generation tasks

### CLI interface

A simple and intuitive command line interface for the user with corresponding options from yaml config
Help system - CLI help and usage examples


## Project Guidelines

### Testing & Documentation

Example configurations - Sample YAML files for different use cases
Integration tests - Tests for the complete workflow from config to output
Unit tests - Unit tests for individual components of logen-rs
Documentation is produced from code


## Code Guidelines

Following rust best practices
