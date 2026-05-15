//! Sensor acquisition subsystem.
//!
//! Deterministic-interval polling of platform sensors that Linux/Go AP
//! cores cannot guarantee timing for.
//!
//! # Sensor types
//!
//! | Class | Interface | Cadence | Notes |
//! |-------|-----------|---------|-------|
//! | CPU/SoC temperature | PECI | 100–500 ms | Latency-sensitive |
//! | Board temperature | I2C (TMP464, LM75) | 1–5 s | Multiple buses, mux-aware |
//! | Voltage/current | I2C PMBus (INA3221) | 1–5 s | Per-board shunt values |
//! | PSU telemetry | CAN bus (TSP only) | 1–5 s | PMBus-over-CAN frames |
//! | Fan tachometer | PWM/Tacho peripheral | 500 ms–1 s | Direct HW access |
//! | Onboard ADC | ADC peripheral | 100 ms–1 s | Board-specific channels |
//!
//! # Data model
//!
//! Each sensor has a compile-time [`SensorDescriptor`] (from `board_config`)
//! and a runtime [`SensorReading`] stored in the shared [`SensorStore`].

/// Sensor type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    Temperature,
    Voltage,
    Current,
    Power,
    FanSpeed,
}

/// Physical interface for sensor access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorInterface {
    Peci,
    I2c,
    Adc,
    Can,
    PwmTacho,
}

/// Engineering unit for sensor values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorUnit {
    DegreesC,
    Volts,
    Amps,
    Watts,
    Rpm,
}

/// Sensor reading status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorStatus {
    Ok,
    Warning,
    Critical,
    Fault,
}
