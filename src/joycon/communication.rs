use std::{
    collections::HashMap,
    fmt::Display,
    net::UdpSocket,
    sync::mpsc,
    time::{Duration, Instant},
};

use itertools::Itertools;
use nalgebra::{UnitQuaternion, Vector3};
use protocol::deku::{DekuContainerRead, DekuContainerWrite};
use protocol::PacketType;
use rosc::{encoder, OscMessage, OscPacket, OscType};

use super::{
    imu::{Imu, JoyconAxisData},
    JoyconDesign,
};
use crate::settings;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Battery {
    Empty,
    Critical,
    Low,
    Medium,
    Full,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub rotation: (f64, f64, f64),
    pub design: JoyconDesign,
    pub serial_number: String,
    pub battery: Battery,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceStatus {
    Healthy,
    LaggyIMU,
    NoIMU,
    Disconnected,
}

impl Display for DeviceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DeviceStatus::Healthy => "Healthy",
            DeviceStatus::LaggyIMU => "Laggy IMU",
            DeviceStatus::NoIMU => "No IMU",
            DeviceStatus::Disconnected => "Disconnected",
        })
    }
}

struct Device {
    imu: Imu,
    design: JoyconDesign,
    send_id: i32,
    battery: Battery,
    status: DeviceStatus,
    imu_times: Vec<Instant>,
}

impl Device {}

#[derive(Debug, Clone)]
pub struct ChannelData {
    pub serial_number: String,
    pub info: ChannelInfo,
}
impl ChannelData {
    pub fn new(serial_number: String, info: ChannelInfo) -> Self {
        Self {
            serial_number,
            info,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChannelInfo {
    Connected(JoyconDesign),
    ImuData([JoyconAxisData; 3]),
    Battery(Battery),
    Reset,
    Disconnected,
}

#[derive(Debug, Copy, Clone)]
struct Xyz {
    x: f64,
    y: f64,
    z: f64,
}

fn calc_acceleration(
    rotation: UnitQuaternion<f64>,
    axisdata: &JoyconAxisData,
    rad_rotation: f64,
) -> Xyz {
    let a = rotation.coords;
    let (x, y, z, w) = (a.x, a.y, a.z, a.w);
    let gravity = [
        2.0 * ((-x) * (-z) - w * y),
        -2.0 * (w * (-x) + y * (-z)),
        w * w - x * x - y * y + z * z,
    ];
    let vector = Xyz {
        x: axisdata.accel_x - gravity[0],
        y: axisdata.accel_y - gravity[1],
        z: axisdata.accel_z - gravity[2],
    };

    let rad_rotation = -rad_rotation;
    Xyz {
        x: vector.x * rad_rotation.cos() - vector.y * rad_rotation.sin(),
        y: vector.x * rad_rotation.sin() + vector.y * rad_rotation.cos(),
        z: vector.z,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServerStatus {
    #[default]
    Disconnected,
    Unknown,
    Connected,
}

pub struct Communication {
    receive: mpsc::Receiver<ChannelData>,
    status_tx: mpsc::Sender<Vec<Status>>,
    server_tx: mpsc::Sender<ServerStatus>,
    settings: settings::Handler,

    devices: HashMap<String, Device>,

    use_keep_ids: bool,
    socket: UdpSocket,
    address: String,
    connected: ServerStatus,
    last_reset: Instant,
}
impl Communication {
    pub fn start(
        receive: mpsc::Receiver<ChannelData>,
        status_tx: mpsc::Sender<Vec<Status>>,
        server_tx: mpsc::Sender<ServerStatus>,
        settings: settings::Handler,
    ) {
        let addrs = [
            SocketAddr::from(([0, 0, 0, 0], 47589)),
            SocketAddr::from(([0, 0, 0, 0], 0)),
        ];
        let socket = UdpSocket::bind(&addrs[..]).unwrap();
        socket.set_nonblocking(true).ok();
        let address = { settings.load().get_socket_address() };
        let use_keep_ids = { settings.load().keep_ids };

        server_tx.send(ServerStatus::Connected).ok();

        Self {
            receive,
            status_tx,
            server_tx,
            settings,
            devices: HashMap::new(),
            use_keep_ids,
            socket,
            address: address.to_string(),
            connected: ServerStatus::Disconnected,

            last_reset: Instant::now(),
        }
        .main_loop();
    }

    fn send_vmt_pose(&self, sensor_id: i32, quat: (f32, f32, f32, f32)) {
        let msg = OscMessage {
            addr: "/VMT/Room/Unity".to_owned(),
            args: vec![
                OscType::Int(sensor_id),
                OscType::Int(1),
                OscType::Float(0.0),
                OscType::Float(0.0),
                OscType::Float(0.0),
                OscType::Float(0.0),
                OscType::Float(quat.0),
                OscType::Float(quat.1),
                OscType::Float(quat.2),
                OscType::Float(quat.3),
            ],
        };
        let packet = OscPacket::Message(msg);
        if let Ok(data) = encoder::encode(&packet) {
            self.socket.send_to(&data, &self.address).ok();
        }
    }

    fn parse_message(&mut self, msg: ChannelData) {
        let sn = msg.serial_number;
        match msg.info {
            ChannelInfo::Connected(design) => {
                if self.devices.contains_key(&sn) {
                    let device = self.devices.get_mut(&sn).unwrap();
                    device.imu = Imu::new();
                    device.imu_times = vec![];
                    return;
                }

                let send_id = if self.use_keep_ids {
                    self.settings.joycon_keep_id(sn.clone()) as i32
                } else {
                    self.devices.len() as i32
                };
                let device = Device {
                    imu: Imu::new(),
                    design,
                    send_id,
                    battery: Battery::Full,
                    status: DeviceStatus::NoIMU,
                    imu_times: vec![],
                };
                self.devices.insert(sn, device);
            }
            ChannelInfo::ImuData(imu_data) => {
                if let Some(device) = self.devices.get_mut(&sn) {
                    for frame in imu_data {
                        device.imu.update(frame);
                    }
                    device.imu_times.push(Instant::now());

                    let joycon_rotation = self.settings.load().joycon_rotation_get(&sn);
                    let rad_rotation = (joycon_rotation as f64).to_radians();
                    let rotated_quat = if joycon_rotation > 0 {
                        device.imu.rotation
                            * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), rad_rotation)
                    } else {
                        device.imu.rotation
                    };

                    let q = rotated_quat.quaternion();
                    self.send_vmt_pose(
                        device.send_id,
                        (q.i as f32, q.j as f32, q.k as f32, q.w as f32),
                    );
                }
            }
            ChannelInfo::Battery(battery) => {
                if let Some(device) = self.devices.get_mut(&sn) {
                    device.battery = battery;
                }
            }
            ChannelInfo::Reset => {
                if self.settings.load().send_reset && self.last_reset.elapsed().as_secs() >= 2 {
                    self.last_reset = Instant::now();
                }
            }
            ChannelInfo::Disconnected => {
                if let Some(device) = self.devices.get_mut(&sn) {
                    device.imu_times = vec![];
                    device.status = DeviceStatus::Disconnected;
                }
            }
        }
    }

    fn update_statuses(&mut self) {
        let discard_before = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
        for device in self.devices.values_mut() {
            device.imu_times.retain(|t| t > &discard_before);
            match device.imu_times.len() {
                x if x >= 55 => {
                    device.status = DeviceStatus::Healthy;
                }
                x if x > 0 => {
                    device.status = DeviceStatus::LaggyIMU;
                }
                _ => {
                    if device.status != DeviceStatus::Disconnected {
                        device.status = DeviceStatus::NoIMU;
                    }
                }
            }
        }
    }

    pub fn main_loop(&mut self) {
        // Spin sleeper with 1ns accuracy. The accuracy is backwards, it means that a request for
        // X sleep will actually sleep for X - 1ns then spin for 1ns max.
        // It is used here because it also sets the minimum Windows sleep time to 1ms instead of 15ms.
        let light_sleeper = spin_sleep::SpinSleeper::new(1)
            .with_spin_strategy(spin_sleep::SpinStrategy::YieldThread);

        let mut last_ui_send = Instant::now();

        loop {
            if self.connected != ServerStatus::Connected {
                self.connected = ServerStatus::Connected;
                self.server_tx.send(self.connected).ok();
            }

            let messages: Vec<_> = self.receive.try_iter().collect();
            if !messages.is_empty() || last_ui_send.elapsed().as_millis() > 100 {
                for msg in messages {
                    self.parse_message(msg);
                }

                self.update_statuses();

                last_ui_send = Instant::now();
                let mut statuses = Vec::new();
                for (serial_number, device) in &self.devices {
                    statuses.push(Status {
                        rotation: device.imu.euler_angles_deg(),
                        design: device.design.clone(),
                        serial_number: serial_number.clone(),
                        battery: device.battery,
                        status: device.status,
                    });
                }
                self.status_tx.send(statuses).ok();
            } else {
                light_sleeper.sleep(Duration::from_millis(2));
            }
        }
    }
}
