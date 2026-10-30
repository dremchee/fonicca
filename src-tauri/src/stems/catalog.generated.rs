// Generated from model-packs/catalog.json. Run pnpm models:generate; do not edit.
// Catalog FNV-1a64: d018ee76adc4a9a9
pub const CLAP_TOTAL: u64 = 34301667;
pub const CLAP_FILE: &str = "clap-htsat-unfused-audio-int8.onnx";
pub const CLAP_URL: &str = "https://huggingface.co/Xenova/clap-htsat-unfused/resolve/c28f2883575e590e04d3146ff0713c2448d691ba/onnx/audio_model_quantized.onnx";
pub const CLAP_SHA256: &str = "3fcff2c8824e7bcb83a983f2a49edab3b60cbcf4872ac70efee517355173bd1f";
pub const TOTAL: u64 = 78856560;
const FILES: [Artifact; 4] = [
    Artifact { name: "vocals.fp16.onnx", size: 19714141, hash: "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca", url: "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/486643021f18a4d057d282fff1d11b89a30e9887/vocals.fp16.onnx" },
    Artifact { name: "drums.fp16.onnx", size: 19714140, hash: "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92", url: "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/486643021f18a4d057d282fff1d11b89a30e9887/drums.fp16.onnx" },
    Artifact { name: "bass.fp16.onnx", size: 19714139, hash: "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55", url: "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/486643021f18a4d057d282fff1d11b89a30e9887/bass.fp16.onnx" },
    Artifact { name: "other.fp16.onnx", size: 19714140, hash: "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d", url: "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/486643021f18a4d057d282fff1d11b89a30e9887/other.fp16.onnx" },
];
pub const SCNET_TOTAL: u64 = 45082486;
const SCNET_FILES: [Artifact; 1] = [
    Artifact { name: "scnet-large.int8.onnx", size: 45082486, hash: "b2dc586a1e0e6c0afe4915e9057ea29111397b7bbf589edc3d85de91de79cd72", url: "https://huggingface.co/audiojs/scnet-large/resolve/95f22d9e68f6cab78aef358f1a0de248d4edf805/scnet-large.int8.onnx" },
];
pub const XL_TOTAL: u64 = 221482626;
const XL_FILES: [Artifact; 1] = [
    Artifact { name: "scnet_xl_wide_v5_fp32.onnx", size: 221482626, hash: "6e2e46b669fa368247afbe867aaa61317425f3f0568309fd6e0c79671c39f0b6", url: "https://huggingface.co/Ryan5453/unblend/resolve/30b749fd691312b1e4b3f8fe79df0b51cde836cd/scnet_xl_wide_v5_fp32.onnx" },
];
pub const DEMUCS_TOTAL: u64 = 316446953;
const DEMUCS_FILES: [Artifact; 1] = [
    Artifact { name: "htdemucs.onnx", size: 316446953, hash: "68d0bf16428ef66e692cdff8a9ccf28f1ef3f69440d57e58605a4cc55fcc5e74", url: "https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/d54ed9eb60e258ea82131c6ee14578628816456a/htdemucs.onnx" },
];
pub const MEL_TOTAL: u64 = 464050097;
const MEL_FILES: [Artifact; 1] = [
    Artifact { name: "mel-roformer.fp16.onnx", size: 464050097, hash: "adcbbb826f12453fbb86652b503242d565a9c81cdef1bca2fe0e54752eba91ef", url: "https://huggingface.co/audiojs/mel-roformer/resolve/c1aba02c00a0bee852d6754fe6ede8164f90a4b3/mel-roformer.fp16.onnx" },
];
pub const BS_TOTAL: u64 = 644830379;
const BS_FILES: [Artifact; 2] = [
    Artifact { name: "bs-roformer-native.onnx", size: 4871339, hash: "320e61bbb30a3bb0cba7b88c013b02b0acef444c3341667158b1bc1611664af9", url: "https://raw.githubusercontent.com/dremchee/fonicca/main/model-packs/v1/bs-roformer-native.onnx" },
    Artifact { name: "bs_roformer_ep317_sdr12.9755.onnx.data", size: 639959040, hash: "3f3677f7ed3801f7415db4b4c900e6e81f4f41d5b4bc83c0bf899c353b2f9995", url: "https://huggingface.co/xycld/BS-RoFormer-ONNX/resolve/f020228f4d25616c8ff4146c339c33687c0be43f/bs_roformer_ep317_sdr12.9755.onnx.data" },
];
pub const MDX_TOTAL: u64 = 448322798;
const MDX_FILES: [Artifact; 2] = [
    Artifact { name: "mdx23c.onnx", size: 221595, hash: "eb6f147119a27b8dd7be4ee8069ef19de115f03656cc054693bccf23c15fa599", url: "https://raw.githubusercontent.com/dremchee/fonicca/main/model-packs/v1/mdx23c.onnx" },
    Artifact { name: "mdx23c.ckpt", size: 448101203, hash: "49d51472769e34a2501cd1da782346a3212555c3a5619fc2c53507445528d816", url: "https://github.com/ZFTurbo/Music-Source-Separation-Training/releases/download/v1.0.0/model_vocals_mdx23c_sdr_10.17.ckpt" },
];
pub const APOLLO_TOTAL: u64 = 66541845;
pub const APOLLO_DIRECTORY: &str = "apollo-v1";
pub const APOLLO_FILE: &str = "pytorch_model.bin";
const APOLLO_FILES: [Artifact; 1] = [
    Artifact { name: "pytorch_model.bin", size: 66541845, hash: "99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76", url: "https://huggingface.co/JusperLee/Apollo/resolve/c68bd80fdd9c0d93d2f4a833cb154624f660a561/pytorch_model.bin" },
];
pub const BASIC_PITCH_TOTAL: u64 = 230444;
pub const BASIC_PITCH_DIRECTORY: &str = "basic-pitch-v1";
pub const BASIC_PITCH_FILE: &str = "nmp.onnx";
const BASIC_PITCH_FILES: [Artifact; 1] = [
    Artifact { name: "nmp.onnx", size: 230444, hash: "2c3c1d144bfa61ad236e92e169c13535c880469a12a047d4e73451f2c059a0ec", url: "https://raw.githubusercontent.com/spotify/basic-pitch/fa5997af0a8210982619003269994a1be25eddf3/basic_pitch/saved_models/icassp_2022/nmp.onnx" },
];
impl Model {
    pub const ALL: [Self; 7] = [Self::Spleeter, Self::ScnetLarge, Self::ScnetXlIhf, Self::Htdemucs, Self::MelRoformer, Self::BsRoformer, Self::Mdx23c];
    pub fn id(self) -> &'static str {
        match self {
            Self::Spleeter => "spleeter",
            Self::ScnetLarge => "scnetLarge",
            Self::ScnetXlIhf => "scnetXlIhf",
            Self::Htdemucs => "htdemucs",
            Self::MelRoformer => "melRoformer",
            Self::BsRoformer => "bsRoformer",
            Self::Mdx23c => "mdx23c",
        }
    }
}
fn files(model: Model) -> &'static [Artifact] {
    match model {
        Model::Spleeter => &FILES,
        Model::ScnetLarge => &SCNET_FILES,
        Model::ScnetXlIhf => &XL_FILES,
        Model::Htdemucs => &DEMUCS_FILES,
        Model::MelRoformer => &MEL_FILES,
        Model::BsRoformer => &BS_FILES,
        Model::Mdx23c => &MDX_FILES,
    }
}
fn directory_name(model: Model) -> &'static str {
    match model {
        Model::Spleeter => "spleeter4-v1",
        Model::ScnetLarge => "scnet-large-v1",
        Model::ScnetXlIhf => "scnet-xl-ihf-v1",
        Model::Htdemucs => "htdemucs-v1",
        Model::MelRoformer => "mel-roformer-kim-v1",
        Model::BsRoformer => "bs-roformer-viperx-v1",
        Model::Mdx23c => "mdx23c-vocals-v1",
    }
}
pub(super) fn tracks(model: Model) -> u8 {
    match model {
        Model::Spleeter => 4,
        Model::ScnetLarge => 4,
        Model::ScnetXlIhf => 4,
        Model::Htdemucs => 4,
        Model::MelRoformer => 2,
        Model::BsRoformer => 2,
        Model::Mdx23c => 2,
    }
}
pub fn total(model: Model) -> u64 {
    match model {
        Model::Spleeter => TOTAL,
        Model::ScnetLarge => SCNET_TOTAL,
        Model::ScnetXlIhf => XL_TOTAL,
        Model::Htdemucs => DEMUCS_TOTAL,
        Model::MelRoformer => MEL_TOTAL,
        Model::BsRoformer => BS_TOTAL,
        Model::Mdx23c => MDX_TOTAL,
    }
}
