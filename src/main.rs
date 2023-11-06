#![allow(unused_imports)]
use std::ptr;
use std::ptr::null_mut;
use std::fs::File;
use std::process::exit;
use std::ffi::{OsStr, CString, CStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use winapi::shared::minwindef::DWORD;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::winspool::PRINTER_ENUM_LOCAL;
use winapi::um::winspool::{PRINTER_INFO_2W, EnumPrintersW};

extern crate simplelog;
extern crate log;

use log::{info, warn, error};
use simplelog::*;
use time::macros::format_description;


#[derive(Clone, Debug)]
struct MinimalPrinterInfo {
    printer_name: OsString,
    port_name: OsString,
    driver_name: OsString,
}

// Convert a null-terminated wide string from raw pointer to Vec<u16>
fn wide_str_from_raw_ptr(ptr: *const u16) -> Vec<u16> {
    let mut length = 0;
    unsafe {
        while *ptr.add(length) != 0 {
            length += 1;
        }
        std::slice::from_raw_parts(ptr, length).to_vec()
    }
}

// Utility function to get the last error
fn get_last_error() -> Option<String> {
    let error_code = unsafe { GetLastError() };

    if error_code == 0 {
        None
    } else {
        let mut buffer: Vec<u16> = Vec::with_capacity(256);
        buffer.resize(buffer.capacity(), 0);
        let len = unsafe {
            winapi::um::winbase::FormatMessageW(
                winapi::um::winbase::FORMAT_MESSAGE_FROM_SYSTEM
                    | winapi::um::winbase::FORMAT_MESSAGE_IGNORE_INSERTS,
                ptr::null(),
                error_code,
                0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                ptr::null_mut(),
            )
        };
        buffer.resize(len as usize, 0);
        Some(OsString::from_wide(&buffer).to_string_lossy().into_owned())
    }
}

fn get_all_printers() -> Vec<MinimalPrinterInfo> {
    // Vector to store MinimalPrinterInfoStructs
    let mut min_printer_info: Vec<MinimalPrinterInfo> = Vec::new();

    let mut bytes_needed: DWORD = 0;
    let mut num_printers: DWORD = 0;

    // First call to EnumPrintersW is to get the number of bytes needed
    info!("[{}] First call to EnumPrintersW to determine bytes_needed", "get_all_printers");
    let enum_printer_result1 = unsafe {
        EnumPrintersW(
            PRINTER_ENUM_LOCAL,
            null_mut(),
            2,
            null_mut(),
            0,
            &mut bytes_needed,
            &mut num_printers,
        )
    };

    if enum_printer_result1 == 0 && bytes_needed == 0 {
        error!("[{}] EnumPrintersW failed to set bytes_needed", "get_all_printers");
        if let Some(win_error) = get_last_error() {
            error!("[{}] EnumPrintersW failed with error code: {}", "get_all_printers", win_error);
        }
        return min_printer_info;
    } else {
        info!("[{}] Bytes needed: {}", "get_all_printers", bytes_needed);
    }

    // Allocate a contiguous block of memory that's large enough to hold all the PRINTER_INFO_2W structs
    let mut buffer = vec![0u8; bytes_needed as usize];

    // Second call to EnumPrintersW receives a pointer to the buffer which EnumPrintersW uses to fill the buffer
    info!("[{}] Second call to EnumPrintersW to populate buffer with PRINTER_INFO_2W structs", "get_all_printers");
    let enum_printer_result2 = unsafe {
        EnumPrintersW(
            PRINTER_ENUM_LOCAL,
            null_mut(),
            2,
            buffer.as_mut_ptr() as *mut _,
            bytes_needed,
            &mut bytes_needed,
            &mut num_printers,
        )
    };

    if enum_printer_result2 == 0 || bytes_needed == 0 {
        error!("[{}] EnumPrintersW failed to populate buffer with PRINTER_INFO_2W structs", "get_all_printers");
        if let Some(win_error) = get_last_error() {
            error!("[{}] EnumPrintersW failed with error code: {}", "get_all_printers", win_error);
        }
        return min_printer_info;
    } else {
        info!("[{}] Successfully filled buffer at {:?}", "get_all_printers", buffer.as_mut_ptr());
    }

    // Transform buffer which is a chunk of raw bytes info a slice of PRINTER_INFO_2W structs
    info!("[{}] Converting raw byte buffer to slice of PRINTER_INFO_2W structs", "get_all_printers");
    let printer_info = unsafe {
        // Cast the buffer pointer to a pointer to PRINTER_INFO_2W.
        let printer_info_ptr = buffer.as_ptr() as *const PRINTER_INFO_2W;

        // With printer_info_ptr being a raw pointer, we now create a slice from the contents it points to
        std::slice::from_raw_parts(printer_info_ptr, num_printers as usize)
    };

    if printer_info.is_empty() {
        warn!("[{}] No printers found", "get_all_printers");
        return min_printer_info;
    } else {
        info!("[{}] Successfully created &[PRINTER_INFO_2W] slice", "get_all_printers");
    }

    // Extract the information needed to create MinimalPrinterInfo struct for each printer
    for printer in printer_info {
        let printer_name = OsString::from_wide(&wide_str_from_raw_ptr(printer.pPrinterName as *const u16));
        let port_name = OsString::from_wide(&wide_str_from_raw_ptr(printer.pPortName as *const u16));
        let driver_name = OsString::from_wide(&wide_str_from_raw_ptr(printer.pDriverName as *const u16));  
        
        let min_printer = MinimalPrinterInfo {
            printer_name: printer_name,
            port_name: port_name,
            driver_name: driver_name,
        };

        min_printer_info.push(min_printer);
    }

    return min_printer_info;
}

fn get_wsd_printers(all_printers: &Vec<MinimalPrinterInfo>) -> Vec<MinimalPrinterInfo> {
    if all_printers.len() == 0 {
        warn!("[{}] Received empty set of printers", "get_wsd_printers");
        return Vec::new();
    }

    // Filter through all_printers and select those whose ports start with WSD
    info!("[{}] Searching through {} printers", "get_wsd_printers", all_printers.len());
    let wsd_printers: Vec<MinimalPrinterInfo> = all_printers.iter()
        .filter(|printer| {
            printer.port_name.to_str()
                .map_or(false, |s| s.starts_with("WSD"))
        })
        .cloned()
        .collect();

    info!("[{}] Successfully found {} WSD connected printers", "get_wsd_printers", wsd_printers.len());

    return wsd_printers;
}

fn main() {
    // Initialize the logger
    let config = ConfigBuilder::new()
        .set_time_format_custom(format_description!("[hour]:[minute]:[second].[subsecond]"))
        .build();

    let _ = WriteLogger::init(LevelFilter::Info, config, File::create("wsd_to_ip.log").expect("Could not create log file"));

    info!("[{}] Getting information from all locally connected printers", "main");
    let all_printers: Vec<MinimalPrinterInfo> = get_all_printers();

    if all_printers.is_empty() {
        warn!("[{}] No printers found", "main");
        return;
    } else {
        info!("[{}] Successfully retrieved printer information", "main");
    }

    let wsd_printers = get_wsd_printers(&all_printers);

    if wsd_printers.len() == 0 {
        warn!("[{}] No WSD connected printers found", "main");
        return;
    }

    for printer in wsd_printers {
        println!("Printer Name: {:?}\n Port Name: {:?}\n Driver Name: {:?}", printer.printer_name, printer.port_name, printer.driver_name);
    }
}