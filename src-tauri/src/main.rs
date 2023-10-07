// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayMenuItem, AppHandle, SystemTrayEvent};
use tauri::async_runtime;
use std::{time, thread, io, time::Duration};
use std::str::from_utf8;
use serialport::{available_ports, SerialPortType, SerialPortInfo};
use hex;
use notify_rust::Notification;
use open;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
// #[tauri::command]
// fn greet(name: &str) -> String {
//     format!("Hello, {}! You've been greeted from Rust!", name)
// }

fn main() {
    let version = CustomMenuItem::new("version".to_string(), "crmbar-tauri 1.0.0").disabled();
    let scanner = CustomMenuItem::new("scanner".to_string(), "сканер не подключён").disabled();
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = SystemTrayMenu::new() // insert the menu items here
        .add_item(version)
        .add_item(scanner)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);
    let system_tray = SystemTray::new()
        .with_menu(tray_menu);
//    let mut join_handle: JoinHandle<()>;
    tauri::Builder::default()
//        .invoke_handler(tauri::generate_handler![greet])
        .system_tray(system_tray)
        .setup(|app| {
            let app_handle = app.handle();
            async_runtime::spawn(async move {
                process_serial(&app_handle);
            });
            Ok(())
        })
        .on_system_tray_event(|_app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => {
                match id.as_str() {
                    "quit" => {
                        std::process::exit(0);
                    },
                    _ => {}
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn process_serial(app_handle: &AppHandle) {
    let host = "https://crm.fsfera.ru/set";
    let baud_rate: u32 = 115200;
    let mut scanner_connected: bool = false;
        
    loop {

        match detect_port() {
            Ok(port_info) => {               
                match serialport::new(&port_info.port_name, baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open() {
                        Ok(mut port) => {
                            if ! scanner_connected {
  //                              let mut manufacturer = "";
                                let mut product = "";
                                match &port_info.port_type {
                                    SerialPortType::UsbPort(info) => {
//                                        manufacturer = info.manufacturer.as_ref().map_or("", String::as_str);
                                        product = info.product.as_ref().map_or("", String::as_str);
                                    }            
                                    SerialPortType::BluetoothPort => {}            
                                    SerialPortType::PciPort => {}            
                                    SerialPortType::Unknown => {}
                                }

                                let _ = Notification::new()
                                    .summary("Подключён сканер штрих-кодов")
                                    .body(&format!("{}\nПорт {}", &product, port_info.port_name))
                                    .appname("rcrmbar")
                                    .timeout(3)
                                    .show();
                                let _ = app_handle.tray_handle().get_item("scanner").set_title(product);
                            }
                            scanner_connected = true;
                            let mut serial_buf: Vec<u8> = vec![0; 1000];
                            println!("Receiving data on {} at {} baud:", &port_info.port_name, &baud_rate);
                            loop {
                                match port.read(serial_buf.as_mut_slice()) {
                                    Ok(t) => {
                                        println!("count: {}",t);
                                        let scan_str = match from_utf8 (&serial_buf[..t]) {
                                            Ok(v) => v.trim(),
                                            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                                        };
                                        let path:String;
                                        if t > 30 {
                                            let encoded = hex::encode(trim_ascii_whitespace(&serial_buf[..t]));
                                            if scan_str.starts_with("http") {
                                                path = format!("{}/select_mdse/?crmbar_version=rcrmbar&wrong_code={}", host, encoded);
                                            } else {
                                                path = format!("{}/select_mdse/?crmbar_version=rcrmbar&hexbarcode={}", host, encoded);
                                            }
                                        } else {
                                            let first_char = scan_str.chars().next().unwrap();
                                            match first_char {
                                                'A' => {path = format!("{}/selectclient/?crmbar_version=rcrmbar&card_id={}",host,&scan_str[1..]);},
                                                'C' => {path = format!("{}/gift_cert/?crmbar_version=rcrmbar&card_id={}",host,&scan_str[1..]);},
                                                'D' => {path = format!("{}/order_barcode/?crmbar_version=rcrmbar&work_id={}",host,&scan_str[1..]);},
                                                'F' => {path = format!("{}/packet_barcode/?crmbar_version=rcrmbar&mode_prod&order_id={}",host,&scan_str[1..]);},
                                                'H' => {path = format!("{}/order_barcode/?crmbar_version=rcrmbar&mode_manager&order_id={}",host,&scan_str[1..]);},
	                                        'G' => {path = format!("{}/order_barcode/?crmbar_version=rcrmbar&calendar=true&work_id={}",host,&scan_str[1..]);},
                                                _ => {path = format!("{}/select_mdse/?crmbar_version=rcrmbar&barcode={}",host,scan_str);}
                                            }
                                        }
                                        println!("{}",path);
                                        match open::that(&path) {
                                            Ok(()) => println!("Opened '{}' successfully.", path),
                                            Err(err) => eprintln!("An error occurred when opening '{}': {}", path, err),
                                        }
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                                    Err(e) => {
                                        eprintln!("{:?}", e);
                                        break;
                                    },
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                            println!("Failed to open \"{}\". Error: {}", port_info.port_name, e);
                            break;
                        }

                    }

            },
            Err(e) => {
                println!("{}",e);

                if scanner_connected {
                    scanner_connected = false;
                    let _ = Notification::new()
                        .summary("Сканер штрих-кодов отключён")
                        .appname("rcrmbar")
                        .timeout(2)
                        .show();
                    let _ = app_handle.tray_handle().get_item("scanner").set_title("Сканнер не подключён");
                }

                let sec = time::Duration::from_millis(2000);
                thread::sleep(sec);
            }
        }
    }
}

/*
fn valid_baud(val: &str) -> Result<(), String> {
val.parse::<u32>()
.map(|_| ())
.map_err(|_| format!("Invalid baud rate '{}' specified", val))
}
*/

pub fn trim_ascii_whitespace(x: &[u8]) -> &[u8] {
    let from = match x.iter().position(|x| !x.is_ascii_whitespace()) {
        Some(i) => i,
        None => return &x[0..0],
    };
    let to = x.iter().rposition(|x| !x.is_ascii_whitespace()).unwrap();
    &x[from..=to]
}

fn detect_port() -> Result<SerialPortInfo, String> {
    let vendors: Vec<u16> = vec![0x1eab,0xa108,0x28e9,0x0c2e,0x0483];
    let ports = available_ports().expect("No ports found!");
    let mut found: Vec<SerialPortInfo> = vec![];
    for ref p in ports {
        if p.port_name.as_str().starts_with("/dev/cu") { continue; }
        match &p.port_type {
            SerialPortType::UsbPort(info) => {
                if !vendors.contains(&info.vid) { continue; }
                found.push(p.clone());                
                println!("Type: USB");
                println!("VID:{:04x} PID:{:04x}", info.vid, info.pid);
                println!("Serial Number: {}", info.serial_number.as_ref().map_or("", String::as_str) );
                println!("Manufacturer: {}", info.manufacturer.as_ref().map_or("", String::as_str) );
                println!("Product: {}", info.product.as_ref().map_or("", String::as_str) );
            }            
            SerialPortType::BluetoothPort => {}            
            SerialPortType::PciPort => {}            
            SerialPortType::Unknown => {}
        }
    }
    match found.first() {
        Some(p) => Ok(p.clone()),
        None => {
            return Err("ports not found".to_string()
            )}
    }
}

