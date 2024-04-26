use std::{
    fs::{self, create_dir_all, remove_dir_all},
    io,
    path::{self, PathBuf},
    process::Command,
};

use coreum_rust_protobuf::transformers;
use prost::Message;
use prost_types::FileDescriptorSet;
use regex::Regex;
use syn::{File, Item};
use walkdir::{DirEntry, WalkDir};

//The directory where proto files will be generated
const OUT_DIR: &str = "transformed-protos";

// version of the Cosmos SDK that we are using
const COSMOS_SDK_VERSION: &str = "v0.47.11";
// version of the WASMD version that we are using
const WASMD_VERSION: &str = "v0.44.0";

const INCLUDE_MODS: [&str; 12] = [
    "/cosmos/auth",
    "/cosmos/authz",
    "/cosmos/bank",
    "/cosmos/base",
    "/cosmos/gov",
    "/cosmos/feegrant",
    "/cosmos/staking",
    "/cosmos/nft",
    "/cosmos/group",
    "/coreum/asset",
    "/coreum/nft",
    "/cosmwasm/wasm",
];

fn main() {
    //Clone the repositories
    let mut cmd = Command::new("git");

    cmd.arg("clone")
        .arg("--branch")
        .arg(COSMOS_SDK_VERSION)
        .arg("git@github.com:cosmos/cosmos-sdk.git");

    cmd.spawn().unwrap().wait().unwrap();

    let mut cmd = Command::new("git");

    cmd.arg("clone")
        .arg("--branch")
        .arg(WASMD_VERSION)
        .arg("git@github.com:CosmWasm/wasmd.git");

    cmd.spawn().unwrap().wait().unwrap();

    let mut cmd = Command::new("git");

    cmd.arg("clone")
        .arg("git@github.com:CoreumFoundation/coreum.git");

    cmd.spawn().unwrap().wait().unwrap();

    //Copy proto files from the repositories here

    let mut cmd = Command::new("cp");
    cmd.arg("-r")
        .arg("./cosmos-sdk/proto")
        .arg("./coreum/proto")
        .arg("./wasmd/proto")
        .arg(".");

    cmd.spawn().unwrap().wait().unwrap();

    //Copy buff.lock for correct dependencies
    let mut cmd = Command::new("cp");
    cmd.arg("buf.lock").arg("./proto");

    cmd.spawn().unwrap().wait().unwrap();

    //Generate rust protobuf files using buf generate and buf build
    let mut cmd_generate = Command::new("buf");
    cmd_generate
        .arg("generate")
        .arg("./proto")
        .arg("--template")
        .arg("buf.gen.yaml")
        .arg("--output")
        .arg("./proto-generated");

    let mut cmd_build = Command::new("buf");
    cmd_build
        .arg("build")
        .arg("./proto")
        .arg("--as-file-descriptor-set")
        .arg("-o")
        .arg("./proto-generated/descriptor.bin");

    if !INCLUDE_MODS.is_empty() {
        for include_mod in INCLUDE_MODS {
            cmd_generate
                .arg("--path")
                .arg(format!("{}{}", "./proto", include_mod));
            cmd_build
                .arg("--path")
                .arg(format!("{}{}", "./proto", include_mod));
        }
    }

    cmd_generate.spawn().unwrap().wait().unwrap();
    cmd_build.spawn().unwrap().wait().unwrap();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path::Path::new("proto-generated"));
    let dir_out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path::Path::new(OUT_DIR));
    remove_dir_all(&dir_out).unwrap_or_default();
    create_dir_all(&dir_out).unwrap();

    let files = fs::read_dir(root.clone())
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()
        .unwrap();

    // filter only files that match "descriptor_*.bin"
    let descriptor_files = files
        .iter()
        .filter(|f| {
            f.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("descriptor")
        })
        .collect::<Vec<_>>();

    // read all files and merge them into one FileDescriptorSet
    let mut file_descriptor_set = FileDescriptorSet { file: vec![] };
    for descriptor_file in descriptor_files {
        let descriptor_bytes = &fs::read(descriptor_file).unwrap()[..];
        let mut file_descriptor_set_tmp = FileDescriptorSet::decode(descriptor_bytes).unwrap();
        file_descriptor_set
            .file
            .append(&mut file_descriptor_set_tmp.file);
    }

    let files: Vec<DirEntry> = WalkDir::new(root.clone())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    for entry in files {
        let file_name = entry
            .file_name()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();

        if file_name.starts_with("descriptor") || file_name.starts_with("buf"){
            continue;
        }

        let mut contents = fs::read_to_string(entry.path()).unwrap();
        for &(regex, replacement) in transformers::REPLACEMENTS {
            contents = Regex::new(regex)
                .unwrap_or_else(|_| panic!("invalid regex: {}", regex))
                .replace_all(&contents, replacement)
                .to_string();
        }

        let file = syn::parse_file(&contents).unwrap();
        let items: Vec<Item> = file
            .items
            .into_iter()
            .map(|i| match i {
                Item::Struct(s) => Item::Struct({
                    let s = transformers::add_derive_eq_struct(&s);
                    let s =
                        transformers::append_attrs_struct(entry.path(), &s, &file_descriptor_set);
                    let s = transformers::serde_alias_id_with_uppercased(s);
                    transformers::allow_serde_int_as_str(s)
                }),

                Item::Enum(e) => Item::Enum({
                    let e = transformers::add_derive_eq_enum(&e);
                    transformers::append_attrs_enum(entry.path(), &e, &file_descriptor_set)
                }),

                // This is a temporary hack to fix the issue with clashing stake authorization validators
                Item::Mod(m) => {
                    Item::Mod(transformers::fix_clashing_stake_authorization_validators(m))
                }

                i => i,
            })
            .collect::<Vec<Item>>();

        let prepended_items = prepend(items);

        contents = prettyplease::unparse(&File {
            items: prepended_items,
            ..file
        });
        fs::write(dir_out.join(file_name), &*contents).unwrap();
    }

    remove_dir_all(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path::Path::new("proto"))).unwrap_or_default();
    remove_dir_all(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path::Path::new("proto-generated"))).unwrap_or_default();
}

fn prepend(items: Vec<Item>) -> Vec<Item> {
    let mut items = items;

    let mut prepending_items = vec![syn::parse_quote! {
        use osmosis_std_derive::CosmwasmExt;
    }];

    items.splice(0..0, prepending_items.drain(..));
    items
}
