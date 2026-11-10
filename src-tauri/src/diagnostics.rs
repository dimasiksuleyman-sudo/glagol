//! Performance diagnostics (0.6.2): hardware facts and timings only.
//!
//! Lines written from here and from the STT/dictation timing sites carry
//! numbers, flags and hardware names — never recognized text, audio, document
//! content or keys (logging rule D-L5, enforced by the source scan in
//! `logging.rs`).
use crate::db::repository::{get_setting, set_setting};
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};

/// `app_settings` key of the «Detailed log» switch. Off by default.
pub const DETAILED_SETTING: &str = "diagnostics_detailed";

static DETAILED: AtomicBool = AtomicBool::new(false);

/// Whether the user turned on «Detailed log».
pub fn detailed() -> bool {
    DETAILED.load(Ordering::Relaxed)
}

/// Read the persisted switch and apply it to the log filter.
pub fn load(conn: &Connection) -> rusqlite::Result<bool> {
    let enabled = get_setting(conn, DETAILED_SETTING)?.as_deref() == Some("1");
    apply(enabled);
    Ok(enabled)
}

/// Persist the switch, then apply it.
pub fn save(conn: &Connection, enabled: bool) -> rusqlite::Result<()> {
    set_setting(
        conn,
        DETAILED_SETTING,
        if enabled { "1" } else { "0" },
        chrono::Utc::now().timestamp_millis(),
    )?;
    apply(enabled);
    Ok(())
}

fn apply(enabled: bool) {
    DETAILED.store(enabled, Ordering::Relaxed);
    let reloaded = crate::logging::set_detailed(enabled);
    tracing::info!(enabled, reloaded, "detailed diagnostics log switch");
}

/// Logical CPUs visible to this process (1 when unknown).
pub fn logical_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// The thread count transcribe.cpp 0.2.3 picks for NULL session params:
/// `min(8, CPUs in the process affinity mask)` (src/transcribe-batch-util.cpp).
/// Logical CPUs approximate the affinity mask, which Glagol never narrows.
pub fn stt_default_threads() -> usize {
    logical_cpus().min(8)
}

/// CPU brand string reported by the processor, if available.
pub fn cpu_brand() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::__cpuid;
        // SAFETY: CPUID is available on every x86_64 processor; leaf 0x8000_0000
        // reports the highest extended leaf, and 0x8000_0002..=0x8000_0004 are
        // read only when that maximum includes them. `__cpuid` has no memory
        // effects. (`allow`: newer toolchains mark the intrinsic safe.)
        #[allow(unused_unsafe)]
        let bytes = unsafe {
            if __cpuid(0x8000_0000).eax < 0x8000_0004 {
                return String::new();
            }
            let mut bytes = Vec::with_capacity(48);
            for leaf in 0x8000_0002u32..=0x8000_0004 {
                let r = __cpuid(leaf);
                for value in [r.eax, r.ebx, r.ecx, r.edx] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes
        };
        String::from_utf8_lossy(&bytes)
            .trim_matches(char::from(0))
            .trim()
            .to_string()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        String::new()
    }
}

/// Instruction-set extensions that decide which ggml CPU variant can load.
pub fn cpu_features() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let mut found = Vec::new();
        macro_rules! probe {
            ($($name:tt),*) => {$(
                if std::arch::is_x86_feature_detected!($name) {
                    found.push($name);
                }
            )*};
        }
        probe!(
            "sse4.2",
            "avx",
            "avx2",
            "fma",
            "f16c",
            "bmi2",
            "avx512f",
            "avx512bw",
            "avx512vnni"
        );
        found.join(",")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        String::new()
    }
}

/// Whether the processor has AVX2, which the fast ggml CPU kernels need.
pub fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// One startup line describing the machine, so logs from two PCs compare.
pub fn log_system() {
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        cpu = %cpu_brand(),
        logical_cpus = logical_cpus(),
        cpu_features = %cpu_features(),
        stt_threads = stt_default_threads(),
        detailed = detailed(),
        "system diagnostics"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrations::apply_migrations(&mut c).unwrap();
        c
    }

    #[test]
    fn detailed_switch_is_off_by_default_and_persists() {
        let c = db();
        assert!(!load(&c).unwrap());
        save(&c, true).unwrap();
        assert!(load(&c).unwrap());
        assert!(detailed());
        save(&c, false).unwrap();
        assert!(!load(&c).unwrap());
        assert!(!detailed());
    }

    #[test]
    fn hardware_probe_is_bounded_and_printable() {
        let brand = cpu_brand();
        assert!(brand.len() <= 48);
        assert!(!brand.contains('\0'));
        assert!((1..=8).contains(&stt_default_threads()));
        #[cfg(target_arch = "x86_64")]
        assert!(cpu_features().split(',').all(|f| !f.is_empty()) || cpu_features().is_empty());
    }
}
