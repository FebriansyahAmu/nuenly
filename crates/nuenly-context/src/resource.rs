//! [`ResourceProfiler`]: memilih model, panjang konteks, dan `keep_alive` dari RAM dan VRAM.

use std::cmp::Reverse;
use std::num::NonZeroU32;
use std::time::Duration;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// GPU dengan VRAM lebih kecil dari ini (misalnya GPU terintegrasi) tidak dihitung.
const MIN_VRAM: u64 = 2 * GIB;
/// Bagian RAM yang boleh dipakai model; sisanya untuk Windows dan aplikasi pengguna.
const RAM_SHARE_PERCENT: u64 = 60;
/// Memori runtime di luar bobot model dan cache konteks.
const RUNTIME_OVERHEAD: u64 = 512 * MIB;
/// Perkiraan cache konteks per token untuk model kelas 4B dengan cache f16.
const KV_PER_TOKEN: u64 = 64 * 1024;

const SMALL_CONTEXT: NonZeroU32 = NonZeroU32::new(4096).unwrap();
const LARGE_CONTEXT: NonZeroU32 = NonZeroU32::new(8192).unwrap();

/// Memori perangkat, dalam byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hardware {
    pub ram: u64,
    /// VRAM GPU terbesar; 0 bila tidak ada GPU.
    pub vram: u64,
}

impl Hardware {
    fn usable_vram(self) -> u64 {
        if self.vram >= MIN_VRAM { self.vram } else { 0 }
    }

    fn ram_budget(self) -> u64 {
        self.ram / 100 * RAM_SHARE_PERCENT
    }
}

/// Model yang terpasang di server model lokal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate<'a> {
    pub name: &'a str,
    /// Ukuran bobot, dalam byte.
    pub size: u64,
}

/// Tempat model berjalan, dari yang tercepat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Placement {
    /// Seluruhnya di GPU.
    Gpu,
    /// Sebagian di GPU, sisanya di prosesor; lebih lambat.
    Split,
    /// Seluruhnya di prosesor.
    Cpu,
    /// Melebihi memori yang wajar dipakai: tetap jalan, tetapi membuat PC berat.
    TooLarge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPlan {
    pub model: String,
    pub placement: Placement,
    pub num_ctx: NonZeroU32,
    /// Lama model tetap dimuat setelah dipakai.
    pub keep_alive: Duration,
    /// Cache konteks `q8_0` (setelan server Ollama) disarankan karena memori sempit.
    pub kv_q8_recommended: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ResourceProfiler {
    hardware: Hardware,
}

impl ResourceProfiler {
    #[must_use]
    pub fn new(hardware: Hardware) -> Self {
        Self { hardware }
    }

    /// Rencana untuk model `preferred`, atau untuk model terbaik bila `None`: penempatan
    /// tercepat lebih dulu, lalu yang terbesar. Model embedding tidak dipilih otomatis.
    ///
    /// `None` bila `preferred` tidak ada di `models`, atau tidak ada model yang bisa dipilih.
    #[must_use]
    pub fn plan(&self, models: &[Candidate<'_>], preferred: Option<&str>) -> Option<ModelPlan> {
        let model = match preferred {
            Some(name) => models.iter().find(|model| model.name == name)?,
            None => models
                .iter()
                .filter(|model| can_chat(model.name))
                .min_by_key(|model| {
                    (
                        self.placement(model.size, SMALL_CONTEXT),
                        Reverse(model.size),
                    )
                })?,
        };

        let placement = self.placement(model.size, SMALL_CONTEXT);
        let num_ctx = if placement == Placement::Gpu
            && self.placement(model.size, LARGE_CONTEXT) == Placement::Gpu
        {
            LARGE_CONTEXT
        } else {
            SMALL_CONTEXT
        };
        Some(ModelPlan {
            model: model.name.to_owned(),
            placement,
            num_ctx,
            keep_alive: keep_alive(placement),
            kv_q8_recommended: num_ctx != LARGE_CONTEXT,
        })
    }

    fn placement(&self, size: u64, context: NonZeroU32) -> Placement {
        let need = size
            .saturating_add(RUNTIME_OVERHEAD)
            .saturating_add(u64::from(context.get()).saturating_mul(KV_PER_TOKEN));
        let vram = self.hardware.usable_vram();
        let ram = self.hardware.ram_budget();
        if vram > 0 && need <= vram {
            Placement::Gpu
        } else if vram > 0 && need <= vram.saturating_add(ram) {
            Placement::Split
        } else if vram == 0 && need <= ram {
            Placement::Cpu
        } else {
            Placement::TooLarge
        }
    }
}

/// Model di GPU dibiarkan dimuat lebih lama agar jawaban berikutnya cepat; model yang memakai
/// RAM cepat dilepas agar aplikasi pengguna tidak ikut lambat.
fn keep_alive(placement: Placement) -> Duration {
    match placement {
        Placement::Gpu => Duration::from_mins(30),
        Placement::Split | Placement::Cpu => Duration::from_mins(5),
        Placement::TooLarge => Duration::from_mins(1),
    }
}

/// Model embedding (misalnya `nomic-embed-text`) tidak bisa diajak bercakap.
fn can_chat(name: &str) -> bool {
    !name.contains("embed")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Candidate, GIB, Hardware, MIB, Placement, ResourceProfiler};

    const GEMMA3_4B: Candidate<'static> = Candidate {
        name: "gemma3:4b",
        size: 3_338_801_804,
    };
    const GEMMA4_E4B: Candidate<'static> = Candidate {
        name: "gemma4:e4b",
        size: 6_583_000_000,
    };
    const GPT_OSS_20B: Candidate<'static> = Candidate {
        name: "gpt-oss:20b",
        size: 13_780_173_839,
    };
    const EMBED: Candidate<'static> = Candidate {
        name: "nomic-embed-text:latest",
        size: 274_302_450,
    };

    /// PC pengembang: RAM 7,7 GB dan RTX 3050 6 GB.
    const DEV_PC: Hardware = Hardware {
        ram: 7_900 * MIB,
        vram: 6 * GIB,
    };

    fn profiler(ram_gib: u64, vram_gib: u64) -> ResourceProfiler {
        ResourceProfiler::new(Hardware {
            ram: ram_gib * GIB,
            vram: vram_gib * GIB,
        })
    }

    #[test]
    fn a_roomy_gpu_gets_a_large_context_and_stays_loaded() {
        let plan = profiler(16, 8).plan(&[GEMMA3_4B], None).unwrap();
        assert_eq!(plan.model, "gemma3:4b");
        assert_eq!(plan.placement, Placement::Gpu);
        assert_eq!(plan.num_ctx.get(), 8192);
        assert_eq!(plan.keep_alive, Duration::from_mins(30));
        assert!(!plan.kv_q8_recommended);
    }

    #[test]
    fn a_tight_gpu_gets_a_small_context() {
        let plan = profiler(16, 4).plan(&[GEMMA3_4B], None).unwrap();
        assert_eq!(plan.placement, Placement::Gpu);
        assert_eq!(plan.num_ctx.get(), 4096);
        assert!(plan.kv_q8_recommended);
    }

    #[test]
    fn a_model_larger_than_vram_is_split() {
        let plan = ResourceProfiler::new(DEV_PC)
            .plan(&[GEMMA4_E4B], None)
            .unwrap();
        assert_eq!(plan.placement, Placement::Split);
        assert_eq!(plan.num_ctx.get(), 4096);
        assert_eq!(plan.keep_alive, Duration::from_mins(5));
        assert!(plan.kv_q8_recommended);
    }

    #[test]
    fn without_a_gpu_the_model_runs_on_the_cpu() {
        let plan = profiler(16, 0).plan(&[GEMMA3_4B], None).unwrap();
        assert_eq!(plan.placement, Placement::Cpu);
        assert_eq!(plan.num_ctx.get(), 4096);
    }

    #[test]
    fn an_integrated_gpu_is_not_counted() {
        let profiler = ResourceProfiler::new(Hardware {
            ram: 16 * GIB,
            vram: 128 * MIB,
        });
        let plan = profiler.plan(&[GEMMA3_4B], None).unwrap();
        assert_eq!(plan.placement, Placement::Cpu);
    }

    #[test]
    fn a_model_beyond_memory_is_too_large() {
        let plan = profiler(8, 0).plan(&[GPT_OSS_20B], None).unwrap();
        assert_eq!(plan.placement, Placement::TooLarge);
        assert_eq!(plan.keep_alive, Duration::from_mins(1));
    }

    #[test]
    fn automatic_choice_prefers_a_model_that_fits_the_gpu() {
        let plan = ResourceProfiler::new(DEV_PC)
            .plan(&[GEMMA4_E4B, GEMMA3_4B], None)
            .unwrap();
        assert_eq!(plan.model, "gemma3:4b");
    }

    #[test]
    fn automatic_choice_prefers_the_larger_of_equally_fast_models() {
        let plan = profiler(32, 24)
            .plan(&[GEMMA3_4B, GPT_OSS_20B], None)
            .unwrap();
        assert_eq!(plan.model, "gpt-oss:20b");
        assert_eq!(plan.placement, Placement::Gpu);
    }

    #[test]
    fn automatic_choice_skips_embedding_models() {
        assert_eq!(profiler(16, 8).plan(&[EMBED], None), None);
        let plan = profiler(16, 8).plan(&[EMBED, GEMMA3_4B], None).unwrap();
        assert_eq!(plan.model, "gemma3:4b");
    }

    #[test]
    fn a_chosen_model_is_planned_even_if_another_is_faster() {
        let plan = ResourceProfiler::new(DEV_PC)
            .plan(&[GEMMA4_E4B, GEMMA3_4B], Some("gemma4:e4b"))
            .unwrap();
        assert_eq!(plan.model, "gemma4:e4b");
        assert_eq!(plan.placement, Placement::Split);
    }

    #[test]
    fn no_plan_without_a_matching_model() {
        assert_eq!(profiler(16, 8).plan(&[], None), None);
        assert_eq!(
            profiler(16, 8).plan(&[GEMMA3_4B], Some("gpt-oss:20b")),
            None
        );
    }
}
