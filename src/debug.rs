/// デバッグ情報を制御するためのモジュール
use std::sync::atomic::{AtomicBool, Ordering};

/// グローバルなデバッグフラグ
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// デバッグモードを設定
pub fn set_debug_mode(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// デバッグモードが有効かどうかを確認
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// デバッグ情報を出力する関数
pub fn debug_print(msg: &str) {
    if is_debug_enabled() {
        eprintln!("🔍 DEBUG: {}", msg);
    }
}

/// エラーデバッグ用の関数
pub fn debug_error(msg: &str) {
    if is_debug_enabled() {
        eprintln!("🔍 DEBUG ERROR: {}", msg);
    }
}

/// 成功デバッグ用の関数
pub fn debug_success(msg: &str) {
    if is_debug_enabled() {
        eprintln!("🔍 DEBUG SUCCESS: {}", msg);
    }
}

/// 警告デバッグ用の関数
pub fn debug_warn(msg: &str) {
    if is_debug_enabled() {
        eprintln!("🔍 DEBUG WARN: {}", msg);
    }
}

/// セパレーター出力用の関数
pub fn debug_separator(label: &str) {
    if is_debug_enabled() {
        eprintln!("🔍 DEBUG: ======== {} ========", label);
    }
}
