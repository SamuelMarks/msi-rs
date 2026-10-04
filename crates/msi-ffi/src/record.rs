//! C-ABI implementation of the Windows Installer Record API.
use msi::database::tables::record::{FieldValue, Record};
use msi::error::MsiError;
use msi::execution::custom_action::{global_handles, MSIHANDLE};

/// Creates a new record object.
#[no_mangle]
pub extern "system" fn MsiCreateRecord(c_params: u32) -> MSIHANDLE {
    std::panic::catch_unwind(|| {
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        // Record format string is field 0
        let fields = vec![FieldValue::Null; (c_params + 1) as usize];
        let record = Record::with_fields(fields);
        hm.register_record(record)
    })
    .unwrap_or(0)
}

/// Retrieves an integer field from a record.
#[no_mangle]
pub extern "system" fn MsiRecordGetInteger(h_record: MSIHANDLE, i_field: u32) -> i32 {
    std::panic::catch_unwind(|| {
        let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let i_field = i_field as usize;

        hm.get_record(h_record).map_or(-2_147_483_648, |record| {
            if i_field < record.fields().len() {
                match record.get(i_field).unwrap_or_else(|| &msi::database::Field::Null) {
                    FieldValue::Long(val) => *val,
                    FieldValue::Short(val) => i32::from(*val),
                    _ => -2_147_483_648, // MSI_NULL_INTEGER
                }
            } else {
                -2_147_483_648
            }
        })
    })
    .unwrap_or(-2_147_483_648)
}

/// Sets an integer field in a record.
#[no_mangle]
pub extern "system" fn MsiRecordSetInteger(h_record: MSIHANDLE, i_field: u32, i_value: i32) -> u32 {
    std::panic::catch_unwind(|| {
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let i_field = i_field as usize;

        hm.get_record_mut(h_record)
            .map_or(MsiError::ERROR_INVALID_HANDLE, |record| {
                if i_field < record.fields().len() {
                    if i_value == -2_147_483_648 {
                        record.fields_mut()[i_field] = FieldValue::Null;
                    } else {
                        record.fields_mut()[i_field] = FieldValue::Long(i_value);
                    }
                    MsiError::ERROR_SUCCESS
                } else {
                    MsiError::ERROR_INVALID_PARAMETER
                }
            })
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Retrieves a string field from a record (UTF-16).
#[no_mangle]
pub unsafe extern "system" fn MsiRecordGetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if pcch_value_buf.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
            let i_field = i_field as usize;

            let string_val = if let Some(record) = hm.get_record(h_record) {
                if i_field < record.fields().len() {
                    match record.get(i_field).unwrap_or_else(|| &msi::database::Field::Null) {
                        FieldValue::String(s) => s.clone(),
                        FieldValue::Long(i) => i.to_string(),
                        FieldValue::Short(i) => i.to_string(),
                        FieldValue::Null => String::new(),
                        FieldValue::Stream(_) => "(stream)".to_string(),
                    }
                } else {
                    String::new()
                }
            } else {
                return MsiError::ERROR_INVALID_HANDLE;
            };

            let utf16_val: Vec<u16> = string_val.encode_utf16().collect();
            let capacity = *pcch_value_buf as usize;

            // +1 for null terminator
            let required_len = utf16_val.len();

            // Set the output length to the string length (not including null terminator per MSI spec)
            *pcch_value_buf = required_len as u32;

            if !sz_value_buf.is_null() {
                if capacity > required_len {
                    std::ptr::copy_nonoverlapping(utf16_val.as_ptr(), sz_value_buf, required_len);
                    *sz_value_buf.add(required_len) = 0;
                    MsiError::ERROR_SUCCESS
                } else {
                    234 /* ERROR_MORE_DATA */
                }
            } else {
                MsiError::ERROR_SUCCESS
            }
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }
}

/// Sets a string field in a record (UTF-16).
#[no_mangle]
pub unsafe extern "system" fn MsiRecordSetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value: *const u16,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
            let i_field = i_field as usize;

            if hm.get_record(h_record).is_none() {
                return MsiError::ERROR_INVALID_HANDLE;
            }

            let string_val = if sz_value.is_null() {
                FieldValue::Null
            } else {
                let mut len = 0;
                while *sz_value.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(sz_value, len);
                match String::from_utf16(slice) {
                    Ok(s) => {
                        if s.is_empty() {
                            FieldValue::Null
                        } else {
                            FieldValue::String(s)
                        }
                    }
                    Err(_) => return MsiError::ERROR_INVALID_PARAMETER,
                }
            };

            let record = match hm.get_record_mut(h_record) { Some(r) => r, None => return MsiError::ERROR_INVALID_HANDLE, };
            if i_field < record.fields().len() {
                record.fields_mut()[i_field] = string_val;
                MsiError::ERROR_SUCCESS
            } else {
                MsiError::ERROR_INVALID_PARAMETER
            }
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }
}

/// Retrieves the size of a record field (stream or string).
#[no_mangle]
pub extern "system" fn MsiRecordDataSize(h_record: MSIHANDLE, i_field: u32) -> u32 {
    std::panic::catch_unwind(|| {
        let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let i_field = i_field as usize;

        hm.get_record(h_record).map_or(0, |record| {
            if i_field < record.fields().len() {
                match record.get(i_field).unwrap_or_else(|| &msi::database::Field::Null) {
                    FieldValue::String(s) => u32::try_from(s.len()).unwrap_or(u32::MAX),
                    FieldValue::Stream(_) | FieldValue::Null => 0,
                    FieldValue::Long(_) | FieldValue::Short(_) => {
                        u32::try_from(size_of::<i32>()).unwrap_or(4)
                    }
                }
            } else {
                0
            }
        })
    })
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_record_api() {
        let h_record = MsiCreateRecord(2);
        assert_ne!(h_record, 0);

        assert_eq!(
            MsiRecordSetInteger(h_record, 1, 42),
            MsiError::ERROR_SUCCESS
        );
        assert_eq!(MsiRecordGetInteger(h_record, 1), 42);

        assert_eq!(
            MsiRecordSetInteger(h_record, 2, -2_147_483_648),
            MsiError::ERROR_SUCCESS
        );
        assert_eq!(MsiRecordGetInteger(h_record, 2), -2_147_483_648);

        let mut len = 0;
        let res = unsafe { MsiRecordGetStringW(h_record, 1, ptr::null_mut(), &mut len) };
        assert_eq!(res, MsiError::ERROR_SUCCESS);
        assert_eq!(len, 2);

        assert_eq!(MsiRecordDataSize(h_record, 1), 4);
    }
}
