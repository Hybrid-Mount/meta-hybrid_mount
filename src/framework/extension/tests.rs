use std::fs;
use std::path::Path;

use super::{Extensions, HookPhase, MountKind};
use crate::framework::config::Config;
use crate::framework::plan::{PlanInput, build_plan};
use crate::framework::scan::list_modules_with;
use crate::mount::model::Mode;
use crate::mount::model::ModuleId;
use crate::mount::model::NodeFileType;
use crate::mount::model::PartitionLayout;
use crate::mount::vfs::rule::{VfsAction, build_vfs_rules};
use crate::test_support::Fixture;

fn module(root: &Path, id: &str) {
    let path = root.join(id);
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("module.prop"),
        format!("id={id}\nname={id}\nversion=1\nauthor=A\ndescription=D\n"),
    )
    .unwrap();
}

#[test]
fn global_hooks_use_isolated_vms_and_collect_each_phase_in_filename_order() {
    let root = Fixture::new("lua-globals");
    assert!(
        Extensions::load(&root.join("missing"), &root)
            .unwrap()
            .commands(HookPhase::Before)
            .unwrap()
            .is_empty()
    );
    fs::write(
        root.join("b.lua"),
        "API = 1\nassert(counter == nil)\nfunction before() overlay_mount('layer', '/dest') end",
    )
    .unwrap();
    fs::write(
        root.join("a.lua"),
        r#"--[=[header]=]
API = 1
assert(_VERSION == 'Lua 5.5' and os == nil and io == nil and package == nil and debug == nil)
assert(not pcall(function() API = 2 end))
assert(vfs_mount == nil and change_mount == nil)
counter = 0
function before() counter = counter + 1; bind_mount(moduledir, '/first') end
function after() assert(counter == 1); vfs_mount('/source', '/last') end
"#,
    )
    .unwrap();
    fs::create_dir(root.join("ignored.lua")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("a.lua"), root.join("link.lua")).unwrap();
    let extensions = Extensions::load(&root, &root).unwrap();
    let before = extensions.commands(HookPhase::Before).unwrap();
    assert_eq!(
        before.iter().map(|mount| mount.kind).collect::<Vec<_>>(),
        [MountKind::Bind, MountKind::Overlay]
    );
    assert_eq!(before[0].source, root.as_ref());
    let after = extensions.commands(HookPhase::After).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].kind, MountKind::Vfs);
    assert_eq!(after[0].target, Path::new("/last"));
}

#[test]
fn lua_path_changes_reach_overlay_layers_magic_nodes_and_encoded_vfs_rules() {
    let scripts = Fixture::new("lua-plan");
    let modules_dir = Fixture::new("lua-modules");
    module(&modules_dir, "demo");
    let source = modules_dir.join("demo");
    for name in ["etc", "lib", "bin"] {
        fs::create_dir_all(source.join("system").join(name)).unwrap();
        fs::write(source.join(format!("system/{name}/sample")), name).unwrap();
    }
    fs::write(
        scripts.join("first.lua"),
        "API = 2\nMODULE = 'demo'\nfunction load() change_mount('system/etc/sample', 'magic') end",
    )
    .unwrap();
    fs::write(
        scripts.join("last.lua"),
        r#"API = 2
MODULE = "demo"
function load()
    assert(bind_mount == nil and overlay_mount == nil and vfs_mount == nil)
    change_mount('system/etc/sample', 'vfs')
    change_mount('system/lib', 'overlay')
    change_mount('system/bin/sample', 'magic')
end
"#,
    )
    .unwrap();
    let layout = PartitionLayout::default();
    let scripts = Extensions::load(&scripts, &modules_dir).unwrap();
    let modules =
        list_modules_with(&modules_dir, &[], |id, _| scripts.module_edits(id, &layout)).unwrap();
    let config = Config {
        default_mode: Mode::Overlay,
        ..Config::default()
    };
    let input = PlanInput {
        modules: &modules,
        config: &config,
        layout: &layout,
        vfs_available: true,
    };
    let plan = build_plan(&input).unwrap();
    assert!(
        plan.overlay_ops
            .iter()
            .any(|op| op.target == "/system/lib" && op.lowerdirs == [source.join("system/lib")])
    );
    assert_eq!(
        plan.tree
            .find("/system/bin/sample")
            .unwrap()
            .source_for(Mode::Magic)
            .unwrap()
            .source_path,
        source.join("system/bin/sample")
    );
    let encoded = crate::mount::vfs::exec::plan_rules(&plan).unwrap();
    assert_eq!(encoded.rules.len(), 1);
    assert_eq!(encoded.rules[0].virtual_path, b"/system/etc/sample");
    assert_eq!(
        encoded.rules[0].real_path,
        source
            .join("system/etc/sample")
            .to_str()
            .unwrap()
            .as_bytes()
    );
    let degraded = build_plan(&PlanInput {
        vfs_available: false,
        ..input
    })
    .unwrap();
    assert!(degraded.vfs_module_ids.is_empty());
    assert_eq!(degraded.magic_module_ids, plan.magic_module_ids);
}

#[test]
fn remove_wins_across_modules_and_block_only_affects_its_owner() {
    let scripts = Fixture::new("lua-remove");
    let modules_dir = Fixture::new("lua-remove-modules");
    for id in ["alpha", "beta"] {
        module(&modules_dir, id);
    }
    let files = modules_dir.join("beta/system/etc");
    fs::create_dir_all(&files).unwrap();
    for name in ["stock", "keep"] {
        fs::write(files.join(name), name).unwrap();
    }
    fs::write(modules_dir.join("replacement"), "new").unwrap();
    fs::write(
        scripts.join("module.lua"),
        r#"API = 2
MODULE = 'alpha'
function load()
    remove('system/etc/stock')
    block('system/etc/keep')
    replace(moduledir .. '/replacement', 'system/etc/new')
end
"#,
    )
    .unwrap();
    let layout = PartitionLayout::default();
    let scripts = Extensions::load(&scripts, &modules_dir).unwrap();
    let modules =
        list_modules_with(&modules_dir, &[], |id, _| scripts.module_edits(id, &layout)).unwrap();
    for mode in [Mode::Magic, Mode::Overlay, Mode::Vfs] {
        let config = Config {
            default_mode: mode,
            ..Config::default()
        };
        let plan = build_plan(&PlanInput {
            modules: &modules,
            config: &config,
            layout: &layout,
            vfs_available: true,
        })
        .unwrap();
        assert_eq!(
            plan.tree
                .find("/system/etc/stock")
                .unwrap()
                .source_for(mode)
                .unwrap()
                .file_type,
            NodeFileType::Whiteout
        );
        assert_eq!(
            plan.tree
                .find("/system/etc/keep")
                .unwrap()
                .source_for(mode)
                .unwrap()
                .module_id,
            "beta"
        );
        assert_eq!(
            plan.tree
                .find("/system/etc/new")
                .unwrap()
                .source_for(mode)
                .unwrap()
                .source_path,
            modules_dir.join("replacement")
        );
        if mode == Mode::Vfs {
            assert!(build_vfs_rules(&plan.tree).iter().any(|rule| matches!(&rule.action, VfsAction::Whiteout { virtual_path } if virtual_path == "/system/etc/stock")));
        }
    }
    assert_eq!(fs::read_to_string(files.join("stock")).unwrap(), "stock");
}

#[test]
fn rejects_invalid_headers_cross_api_calls_and_out_of_scope_paths() {
    let root = Fixture::new("lua-validation");
    let path = root.join("script.lua");
    for script in [
        "API = 3",
        "API = 2\nMODULE = '../bad'",
        "API = 2\nMODULE = 'ok'\nfunction load(arg) end",
        "API = 1\nfunction load() end",
    ] {
        fs::write(&path, script).unwrap();
        assert!(Extensions::load(&root, &root).is_err(), "{script}");
    }
    let id = ModuleId::try_from("demo").unwrap();
    let layout = PartitionLayout::default();
    for body in [
        "change_mount('/system/etc', 'vfs')",
        "change_mount('system/../etc', 'magic')",
        "change_mount('system', 'ignore')",
        "remove('system')",
        "bind_mount('a', '/b')",
        "MODULE = 'other'",
    ] {
        fs::write(
            &path,
            format!("API = 2\nMODULE = 'demo'\nfunction load() {body} end"),
        )
        .unwrap();
        let scripts = Extensions::load(&root, &root).unwrap();
        assert!(scripts.module_edits(&id, &layout).is_err(), "{body}");
    }
    fs::write(
        &path,
        "API = 2\nMODULE = 'demo'\nfunction load() skip(); change_mount('bad','value') end",
    )
    .unwrap();
    fs::write(
        root.join("z.lua"),
        "API = 2\nMODULE = 'demo'\nfunction load() error('after skip') end",
    )
    .unwrap();
    assert!(
        Extensions::load(&root, &root)
            .unwrap()
            .module_edits(&id, &layout)
            .unwrap()
            .skip
    );
}
