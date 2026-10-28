"""Check the production updater comparison without compiling the GUI test suite."""
import json
from pathlib import Path
import re
import subprocess


def block(source, declaration):
    start = source.index(declaration)
    return source[start:source.index("\n}", start) + 2]


def main():
    root = Path("dist")
    build = json.loads((root / "candidate-build.json").read_text(encoding="utf-8-sig"))
    reported = (root / "candidate-version.txt").read_text(encoding="utf-8-sig")
    match = re.search(r"Local Qwen AI (\d{6}-(?:(?:[2-9]|[1-9]\d+)-)?\d+\.\d+\.\d+)", reported)
    assert match and match[1] == "261028-2-0.16.0", reported
    tag = match[1]
    name = f"photocraft-qwen-edit-{tag}-windows-x64.zip"
    assert build["zip"] == name
    updater = Path("apps/photocraft/src/updater.rs").read_text(encoding="utf-8")
    ui = Path("crates/ui-egui/src/updates.rs").read_text(encoding="utf-8")
    engine = Path("crates/engine/src/build_info.rs").read_text(encoding="utf-8")
    repo = re.search(r'pub const FORK_REPOSITORY: &str = "([^"]+)";', engine)[1]
    code = "#[derive(Debug)]\n" + block(ui, "pub struct Release {") + "\n"
    code += block(updater, "struct Asset {") + "\n" + block(updater, "struct ApiRelease {") + "\n"
    code += f"const REPO: &str = {json.dumps(repo)};\n"
    code += re.search(r"const MAX_ARCHIVE: u64 = [^;]+;", updater)[0] + "\n"
    for fn in ("pub(crate) fn release_key(", "fn digest(", "fn trusted_asset_url(", "fn select("):
        code += block(updater, fn) + "\n"
    code += r'''
fn fixture(tag: &str, name: &str, hash: &str) -> ApiRelease {
    ApiRelease { tag_name: tag.into(), draft: false, prerelease: false, assets: vec![
        Asset { name: name.into(), browser_download_url: format!("https://github.com/{REPO}/releases/download/{tag}/{name}"), size: 100, digest: Some(format!("sha256:{hash}")) },
        Asset { name: format!("superseded-embedded261026-{name}"), browser_download_url: String::new(), size: 100, digest: None }
    ] }
}
'''
    code += "fn main() {\n"
    code += f"let tag = {json.dumps(tag)}; let name = {json.dumps(name)}; let hash = {json.dumps(build['zip_sha256'])};\n"
    code += r'''
    let selected = select(fixture(tag, name, hash), "261028-0.16.0").unwrap().unwrap();
    assert_eq!(selected.tag, tag);
    assert_eq!(selected.asset_name, name);
    assert_eq!(selected.sha256, hash);
    assert!(select(fixture(tag, name, hash), tag).unwrap().is_none());
    let mut draft = fixture(tag, name, hash); draft.draft = true;
    assert!(select(draft, "261028-0.16.0").unwrap().is_none());
    println!("Production updater selects the corrected asset; equal installed/latest versions offer no update.");
}
'''
    probe = root / "updater-identity-check.rs"
    probe.write_text(code, encoding="utf-8")
    binary = root / "updater-identity-check.exe"
    subprocess.run(["rustc", "--edition=2024", str(probe), "-o", str(binary)], check=True)
    subprocess.run([str(binary.resolve())], check=True)
    result = dict(installed_tag=tag, latest_tag=tag, selected_asset=name,
                  selected_sha256=build["zip_sha256"], old_installation_offered=True,
                  equal_version_offer=False, superseded_asset_ignored=True,
                  product_source_sha=build["source_sha"],
                  method="Compiled unchanged production release_key/digest/trusted_asset_url/select functions with release fixtures; executable identity read from --version")
    (root / "updater-identity-verification.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
