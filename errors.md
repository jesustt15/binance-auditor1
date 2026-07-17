
C:\Users\Usuario\Documents\binance-auditor1>cargo run --manifest-path server/Cargo.toml
warning: unused imports: `Deserialize` and `Serialize`
 --> src\config.rs:2:13
  |
2 | use serde::{Deserialize, Serialize};
  |             ^^^^^^^^^^^  ^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused imports: `DateTime` and `Duration`
 --> src\imap_sync.rs:1:14
  |
1 | use chrono::{DateTime, Duration, Utc};
  |              ^^^^^^^^  ^^^^^^^^

warning: unused import: `mailparse::parse_mail`
 --> src\imap_sync.rs:2:5
  |
2 | use mailparse::parse_mail;
  |     ^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `uuid::Uuid`
 --> src\imap_sync.rs:5:5
  |
5 | use uuid::Uuid;
  |     ^^^^^^^^^^

warning: unused import: `crate::models::FraudResult`
  --> src\imap_sync.rs:10:5
   |
10 | use crate::models::FraudResult;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: variable does not need to be mutable
   --> src\db.rs:195:9
    |
195 |     let mut query = sqlx::query_as::<_, Payment>(
    |         ----^^^^^
    |         |
    |         help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: variable does not need to be mutable
   --> src\db.rs:246:9
    |
246 |     let mut query = sqlx::query_as::<_, Payment>(
    |         ----^^^^^
    |         |
    |         help: remove this `mut`

warning: variable does not need to be mutable
   --> src\db.rs:481:9
    |
481 |     let mut sql = String::from(
    |         ----^^^
    |         |
    |         help: remove this `mut`

warning: unused import: `std::io::Write`
    --> src\routes\mod.rs:1193:9
     |
1193 |     use std::io::Write;
     |         ^^^^^^^^^^^^^^

warning: unused variable: `dmarc`
   --> src\anti_fraud.rs:163:52
    |
163 | fn determine_failure_reason(dkim: bool, spf: bool, dmarc: bool) -> String {
    |                                                    ^^^^^ help: if this is intentional, prefix it with an underscore: `_dmarc`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `sql`
   --> src\db.rs:481:9
    |
481 |     let mut sql = String::from(
    |         ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_sql`

warning: variable does not need to be mutable
   --> src\imap_sync.rs:381:9
    |
381 |     let mut nuevos = 0i64;
    |         ----^^^^^^
    |         |
    |         help: remove this `mut`

warning: variable does not need to be mutable
   --> src\imap_sync.rs:382:9
    |
382 |     let mut duplicados = 0i64;
    |         ----^^^^^^^^^^
    |         |
    |         help: remove this `mut`

warning: variable does not need to be mutable
   --> src\imap_sync.rs:383:9
    |
383 |     let mut en_cuarentena = 0i64;
    |         ----^^^^^^^^^^^^^
    |         |
    |         help: remove this `mut`

warning: value assigned to `buf` is never read
    --> src\routes\mod.rs:1189:28
     |
1189 |     let mut buf: Vec<u8> = Vec::new();
     |                            ^^^^^^^^^^ this value is reassigned later and never used
...
1199 |     buf = std::fs::read(&tmp_path).map_err(|e| format!("Read temp xlsx error: {}", e))?;
     |     --- `buf` is overwritten here before the previous value is read
     |
     = note: `#[warn(unused_assignments)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `cert_pem`
  --> src\main.rs:89:17
   |
89 |             let cert_pem = std::fs::read_to_string(cert)?;
   |                 ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cert_pem`

warning: unused variable: `key_pem`
  --> src\main.rs:90:17
   |
90 |             let key_pem = std::fs::read_to_string(key)?;
   |                 ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_key_pem`

warning: constant `DKIM_HEADER` is never used
 --> src\anti_fraud.rs:6:7
  |
6 | const DKIM_HEADER: &str = "DKIM-Signature";
  |       ^^^^^^^^^^^
  |
  = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: constant `SPF_HEADER` is never used
 --> src\anti_fraud.rs:7:7
  |
7 | const SPF_HEADER: &str = "Received-SPF";
  |       ^^^^^^^^^^

warning: constant `DMARC_HEADER` is never used
 --> src\anti_fraud.rs:8:7
  |
8 | const DMARC_HEADER: &str = "Authentication-Results";
  |       ^^^^^^^^^^^^

warning: function `verify_email_headers` is never used
  --> src\anti_fraud.rs:14:8
   |
14 | pub fn verify_email_headers(raw_email: &[u8]) -> FraudResult {
   |        ^^^^^^^^^^^^^^^^^^^^

warning: function `determine_failure_reason` is never used
   --> src\anti_fraud.rs:163:4
    |
163 | fn determine_failure_reason(dkim: bool, spf: bool, dmarc: bool) -> String {
    |    ^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `check_duplicate` is never used
   --> src\anti_fraud.rs:174:8
    |
174 | pub fn check_duplicate(
    |        ^^^^^^^^^^^^^^^

warning: function `check_amount_tamper` is never used
   --> src\anti_fraud.rs:190:8
    |
190 | pub fn check_amount_tamper(
    |        ^^^^^^^^^^^^^^^^^^^

warning: field `username` is never read
   --> src\auth.rs:106:9
    |
104 | pub struct AuthUser {
    |            -------- field in this struct
105 |     pub user_id: String,
106 |     pub username: String,
    |         ^^^^^^^^
    |
    = note: `AuthUser` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: function `require_auth_any` is never used
   --> src\auth.rs:148:8
    |
148 | pub fn require_auth_any(_auth: &AuthUser) -> Result<(), AuthError> {
    |        ^^^^^^^^^^^^^^^^

warning: function `insert_payment` is never used
   --> src\db.rs:358:14
    |
358 | pub async fn insert_payment(
    |              ^^^^^^^^^^^^^^

warning: function `payment_exists_by_uid` is never used
   --> src\db.rs:395:14
    |
395 | pub async fn payment_exists_by_uid(
    |              ^^^^^^^^^^^^^^^^^^^^^

warning: function `payment_exists_by_fields` is never used
   --> src\db.rs:409:14
    |
409 | pub async fn payment_exists_by_fields(
    |              ^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `insert_quarantined_email` is never used
   --> src\db.rs:513:14
    |
513 | pub async fn insert_quarantined_email(
    |              ^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `insert_import_row` is never used
   --> src\db.rs:666:14
    |
666 | pub async fn insert_import_row(
    |              ^^^^^^^^^^^^^^^^^

warning: constant `IMAP_HOST` is never used
  --> src\imap_sync.rs:12:7
   |
12 | const IMAP_HOST: &str = "imap.gmail.com";
   |       ^^^^^^^^^

warning: constant `IMAP_PORT` is never used
  --> src\imap_sync.rs:13:7
   |
13 | const IMAP_PORT: u16 = 993;
   |       ^^^^^^^^^

warning: function `html_to_text` is never used
  --> src\imap_sync.rs:20:4
   |
20 | fn html_to_text(html: &str) -> String {
   |    ^^^^^^^^^^^^

warning: function `is_label` is never used
  --> src\imap_sync.rs:67:4
   |
67 | fn is_label(line: &str, candidates: &[&str]) -> bool {
   |    ^^^^^^^^

warning: function `subject_matches_deposit` is never used
  --> src\imap_sync.rs:73:4
   |
73 | fn subject_matches_deposit(subject: &str) -> bool {
   |    ^^^^^^^^^^^^^^^^^^^^^^^

warning: function `parse_time_field` is never used
  --> src\imap_sync.rs:80:4
   |
80 | fn parse_time_field(s: &str) -> Option<String> {
   |    ^^^^^^^^^^^^^^^^

warning: function `parse_amount_field` is never used
  --> src\imap_sync.rs:87:4
   |
87 | fn parse_amount_field(s: &str) -> Option<(f64, String)> {
   |    ^^^^^^^^^^^^^^^^^^

warning: function `extract_deposit_data` is never used
  --> src\imap_sync.rs:99:4
   |
99 | fn extract_deposit_data(text: &str, subject: &str) -> Option<crate::models::RawEmailData> {
   |    ^^^^^^^^^^^^^^^^^^^^

warning: function `extract_binance_data` is never used
   --> src\imap_sync.rs:143:4
    |
143 | fn extract_binance_data(text: &str, subject: &str) -> Option<crate::models::RawEmailData> {
    |    ^^^^^^^^^^^^^^^^^^^^

warning: function `get_text_body` is never used
   --> src\imap_sync.rs:200:4
    |
200 | fn get_text_body(parsed: &mailparse::ParsedMail) -> String {
    |    ^^^^^^^^^^^^^

warning: function `get_text_body_recursive` is never used
   --> src\imap_sync.rs:204:4
    |
204 | fn get_text_body_recursive(parsed: &mailparse::ParsedMail, depth: usize) -> String {
    |    ^^^^^^^^^^^^^^^^^^^^^^^

warning: function `process_synced_emails` is never used
   --> src\imap_sync.rs:432:14
    |
432 | pub async fn process_synced_emails(
    |              ^^^^^^^^^^^^^^^^^^^^^

warning: enum `FraudVerdict` is never used
  --> src\models.rs:52:10
   |
52 | pub enum FraudVerdict {
   |          ^^^^^^^^^^^^

warning: enum `ReviewDecision` is never used
  --> src\models.rs:86:10
   |
86 | pub enum ReviewDecision {
   |          ^^^^^^^^^^^^^^

warning: struct `Import` is never constructed
   --> src\models.rs:264:12
    |
264 | pub struct Import {
    |            ^^^^^^

warning: struct `ImportRow` is never constructed
   --> src\models.rs:276:12
    |
276 | pub struct ImportRow {
    |            ^^^^^^^^^

warning: struct `LoginResponse` is never constructed
   --> src\models.rs:299:12
    |
299 | pub struct LoginResponse {
    |            ^^^^^^^^^^^^^

warning: struct `RefreshResponse` is never constructed
   --> src\models.rs:311:12
    |
311 | pub struct RefreshResponse {
    |            ^^^^^^^^^^^^^^^

warning: struct `VerifyPaymentResponse` is never constructed
   --> src\models.rs:340:12
    |
340 | pub struct VerifyPaymentResponse {
    |            ^^^^^^^^^^^^^^^^^^^^^

warning: struct `SyncStatusResponse` is never constructed
   --> src\models.rs:367:12
    |
367 | pub struct SyncStatusResponse {
    |            ^^^^^^^^^^^^^^^^^^

warning: struct `TriggerSyncResponse` is never constructed
   --> src\models.rs:380:12
    |
380 | pub struct TriggerSyncResponse {
    |            ^^^^^^^^^^^^^^^^^^^

warning: struct `ImportResultResponse` is never constructed
   --> src\models.rs:395:12
    |
395 | pub struct ImportResultResponse {
    |            ^^^^^^^^^^^^^^^^^^^^

warning: struct `FraudResult` is never constructed
   --> src\models.rs:444:12
    |
444 | pub struct FraudResult {
    |            ^^^^^^^^^^^

warning: struct `RawEmailData` is never constructed
   --> src\models.rs:453:12
    |
453 | pub struct RawEmailData {
    |            ^^^^^^^^^^^^

warning: struct `ClientModeConfig` is never constructed
   --> src\models.rs:466:12
    |
466 | pub struct ClientModeConfig {
    |            ^^^^^^^^^^^^^^^^

warning: `binance-auditor-server` (bin "binance-auditor-server") generated 56 warnings (run `cargo fix --bin "binance-auditor-server" -p binance-auditor-server` to apply 15 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.47s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running `server\target\debug\binance-auditor-server.exe`
2026-07-17T19:26:43.886372Z  INFO binance_auditor_server: Iniciando Binance Auditor Server v1.0...
[WARN] Usando JWT_SECRET por defecto. Configuralo en produccion!
2026-07-17T19:26:43.886782Z  INFO binance_auditor_server: Config cargada: listen=0.0.0.0:8443, db=localhost:5432/auditor_db
2026-07-17T19:26:43.887030Z  INFO binance_auditor_server: age identity cargada de C:\Users\Usuario\AppData\Roaming\binance-auditor-server\age_key.txt
Error: "DB pool error: error returned from database: password authentication failed for user \"auditor\""
error: process didn't exit successfully: `server\target\debug\binance-auditor-server.exe` (exit code: 1)