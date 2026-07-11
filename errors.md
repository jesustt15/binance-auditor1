error[E0277]: the trait bound `i64: rust_xlsxwriter::IntoExcelData` is not satisfied
   --> src\db.rs:204:33
    |
204 |         worksheet.write(row, 0, pago.id).map_err(|e| format!("Excel write error: {}", e))?;
    |                   -----         ^^^^^^^ the trait `rust_xlsxwriter::IntoExcelData` is not implemented for `i64`
    |                   |
    |                   required by a bound introduced by this call
    |
    = help: the following other types implement trait `rust_xlsxwriter::IntoExcelData`:
              f32
              f64
              i16
              i32
              i8
              u16
              u32
              u8
note: required by a bound in `Worksheet::write`
   --> C:\Users\Jesus\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\rust_xlsxwriter-0.40.0\src\worksheet.rs:559:20
    |
555 |     pub fn write(
    |            ----- required by a bound in this associated function
...
559 |         data: impl IntoExcelData,
    |                    ^^^^^^^^^^^^^ required by this bound in `Worksheet::write`

For more information about this error, try `rustc --explain E0277`.
error: could not compile `binance-auditor` (bin "binance-auditor") due to 1 previous error