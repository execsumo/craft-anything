use super::check::build_check;
use super::*;

#[path = "note_progress_tests.rs"]
mod note_progress;

#[path = "stall_tests.rs"]
mod stall;

// Helpers that take a built-in kind's id, so each test reads as one line.
fn kind(id: &str) -> Arc<dyn TargetKind> {
    KindRegistry::built_in().get(id).unwrap()
}
fn choose(request: &str, target: &TargetRef) -> (&'static str, &'static str) {
    let (k, stack) = KindRegistry::built_in().choose(request, target, true).unwrap();
    (k.id(), stack)
}
fn kind_of(request: &str, target: &TargetRef, stack: &str) -> &'static str {
    KindRegistry::built_in().detect(request, target, stack).unwrap().id()
}
fn brief(ws: &Workspace, k: &str, deep: bool, ghidra: bool, editor: Option<&Path>, assets: bool, extra: Option<&str>) -> String {
    super::brief(kind(k).as_ref(), &BriefCx { ws, deep, ghidra, editor, assets, image_model: None, sees_pictures: true }, extra)
}
fn refine(k: &str, check: &CheckResult) -> String {
    kind(k).refine(check, true)
}
fn follow_up(message: &str, k: &str) -> String {
    super::follow_up(message, kind(k).as_ref(), false)
}
fn locate(path: &Path, k: &str) -> (PathBuf, Option<PathBuf>) {
    super::locate(path, kind(k).studies_folder())
}

#[test]
fn stack_choice() {
    let web = TargetRef::Website { url: "https://a.com".into() };
    assert_eq!(choose("Clone this website", &web), ("website", stacks::BUILDERS_CHOICE), "the builder picks a website's stack, like the original's");
    assert_eq!(choose("Clone this website with React", &web).1, "React + TypeScript (Vite)");
    assert_eq!(choose(SAID_NEXT_SHADCN, &web), ("website", "Next.js + TypeScript"), "the stack the person named, not the default");
    assert_eq!(choose("Recreate this Steam game's gameplay", &TargetRef::None), ("game", "Unity"), "games default to Unity");
    assert_eq!(choose("Rebuild this desktop application in Rust", &TargetRef::None), ("app", "Rust + egui (eframe)"));
    assert_eq!(choose("make a tool in python", &TargetRef::None).1, "Python");
    assert_eq!(choose("Recreate this game's core gameplay as a small game in Rust", &TargetRef::None).1, "Rust + macroquad");
    assert_eq!(choose("A platformer game with Godot", &TargetRef::None).1, "Godot");
    assert_eq!(choose("Remake this game in Unreal Engine 5", &TargetRef::None).1, "Unreal Engine");
    assert_eq!(choose("A snake game using unity", &TargetRef::None).1, "Unity");
    assert_eq!(choose("A pirate game in rustic colours", &TargetRef::None).1, "Unity", "a word that only starts like an engine doesn't count");
    assert_eq!(choose("A racing game, made in Rust.", &TargetRef::None).1, "Rust + macroquad");
    assert_eq!(choose("a landing page for my bakery", &TargetRef::None).0, "website");
    // The request names how to build: that wins over every kind's default.
    assert_eq!(choose("A notes app in Electron", &TargetRef::None), ("app", "Electron"));
    assert_eq!(choose("An Electron app with React and shadcn", &TargetRef::None).1, "Electron", "the app's own stack before its page library");
    assert_eq!(choose("A racing game in Electron", &TargetRef::None), ("game", "Electron"));
    let kinds = KindRegistry::built_in();
    assert_eq!(kinds.named("A racing game in Godot", &TargetRef::None), Some("Godot"));
    assert_eq!(kinds.named("a music player, built with electron", &TargetRef::None), Some("Electron"));
    assert_eq!(kinds.named("A racing game", &TargetRef::None), None, "nothing named: each kind's default applies");
    // Built with a game engine means a game, even when the request never says "game".
    let folder = TargetRef::Local { path: PathBuf::from("D:/Games/Untitled Project") };
    assert_eq!(choose("Clone Untitled Project. Match the original as closely as possible 1:1.", &folder).0, "app");
    assert_eq!(kind_of("Clone Untitled Project. Match the original as closely as possible 1:1.", &folder, "Unreal Engine"), "game");
    assert_eq!(kind_of("Copy this tool", &folder, "Rust + egui (eframe)"), "app", "the app stack stays an app");
}

#[test]
fn the_word_player_alone_is_not_a_game() {
    for request in [
        "A video player for my films",
        "a music player",
        "Um player de vídeo com canais, filmes e séries",
        "An MPEG-2 player",
        "an mpeg-2-player",
        "an mp2 player",
        "an mp4 player",
        "a 12 player scoreboard",
        "a music app with two playerctl shortcuts",
        "a low-level disk tool",
        "a low level disk tool",
        "a high level system monitor",
        "An MPEG 2 player",
        "An MPEG  2-player",
        "a single player app for movies",
        "a single-player application for music",
        "a single playerctl shortcut",
        "a multilevel menu",
    ] {
        assert_eq!(choose(request, &TargetRef::None).0, "app", "{request}");
    }
    assert_eq!(choose("Um player de vídeo desktop, use Electron", &TargetRef::None), ("app", "Electron"));
    for request in [
        "A multiplayer shooter",
        "A single-player adventure",
        "A single player adventure",
        "a 2-player chess board",
        "a two player chess board",
        "Build a (2-player) chess board",
        "a 2  player chess board",
        "chess for 2 players",
        "party fun for four players.",
        "a two\nplayer chess board",
        "a four-player party",
        "4 player dungeon crawl",
        "A game with one player",
        "A level editor",
        "Build a (level editor)",
        "a low level tool with a level editor",
        "a game with an MPEG 2 player",
        "a single player app for chess",
        "Make this:\nlevel editor",
    ] {
        assert_eq!(choose(request, &TargetRef::None).0, "game", "{request}");
    }
}

#[test]
fn italian_requests_keep_their_project_kind() {
    for request in ["Un semplice gioco del serpente", "Per esempio: lo stesso gioco, con una classifica", "Fai un gioco che si gioca così"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["Un sito per la mia panetteria", "Ricostruisci questo sito", "Per esempio: ricostruisci questo sito"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
}

#[test]
fn german_requests_keep_their_project_kind() {
    for request in ["Ein einfaches Snake-Spiel", "Dasselbe Spiel mit einer Bestenliste"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["Eine Website für meine Bäckerei", "Diese Webseite nachbauen"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
}

#[test]
/// A request written in Spanish keeps the project kind its English equivalent would get, which is
/// what the German and Chinese cases assert for their own words.
fn spanish_requests_keep_their_project_kind() {
    for request in ["Un juego de serpiente sencillo", "El mismo juego, con una tabla de récords"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["Un sitio web para mi panadería", "Recrea este sitio web", "Una página web de mi taller"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
    assert_eq!(choose("La misma app, pero más simple", &TargetRef::None).0, "app");
    // "juego" also names a set: a tool set is not a game.
    assert_eq!(choose("Una app para llevar el inventario de mi juego de herramientas", &TargetRef::None).0, "app");
    // "web" alone says nothing: a web app is built as an app, not as a website.
    for request in ["Una aplicación web de notas", "A web app in Rust"] {
        assert_eq!(choose(request, &TargetRef::None).0, "app", "{request}");
    }
}

#[test]
fn portuguese_and_turkish_requests_keep_their_project_kind() {
    for request in ["Um jogo simples da cobrinha", "Basit bir yılan oyunu", "Bir oyun yap"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    assert_eq!(choose("Bir web sitesi yap", &TargetRef::None), ("website", stacks::BUILDERS_CHOICE));
    assert_eq!(choose("Um site para a minha padaria", &TargetRef::None), ("website", stacks::BUILDERS_CHOICE));
    assert_eq!(choose("Bir oyuncak mağazası uygulaması", &TargetRef::None).0, "app", "oyuncak is a toy");
}

#[test]
fn vietnamese_requests_keep_their_project_kind() {
    for request in ["Một trò chơi rắn săn mồi đơn giản", "Trò chơi đó, thêm bảng xếp hạng"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["Một trang web cho tiệm bánh của tôi", "Dựng lại Trang web này"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
}

#[test]
fn thai_requests_keep_their_project_kind() {
    for request in ["เกมงูกินหางง่าย ๆ", "เกมเดิม แต่มีตารางคะแนนสูงสุด"]
    {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["เว็บไซต์ร้านเบเกอรีของฉัน", "สร้างเว็บไซต์นี้ใหม่"]
    {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
    assert_eq!(choose("แอปเครื่องคิดเลข", &TargetRef::None).0, "app", "แอปเครื่องคิดเลข");
}

#[test]
fn estonian_requests_keep_their_project_kind() {
    for request in ["Lihtne usumäng", "Sama mäng, aga edetabeliga", "Loo mäng, mis mängib nii", "Loo mänge lastele"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["Veebileht minu pagaritöökojale", "Loo see veebileht uuesti", "Veebileht minu töökojale"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
    assert_eq!(choose("Sama rakendus, aga lihtsam", &TargetRef::None).0, "app");
    // A game noun keeps its kind in compounds and inflected forms; the verb "mängima" (to play)
    // and toy compounds do not.
    for request in ["Lauamäng", "Arvutimängu uus versioon"] {
        assert_eq!(choose(request, &TargetRef::None).0, "game", "{request}");
    }
    for request in ["Loo muusikat mängiv rakendus", "Mänguasjapood"] {
        assert_eq!(choose(request, &TargetRef::None).0, "app", "{request}");
    }
    // Punctuation and newlines still expose the website cues, and a browser is not a website.
    for request in ["Loo:\nveebileht pagaritöökojale", "Kodulehele"] {
        assert_eq!(choose(request, &TargetRef::None).0, "website", "{request}");
    }
    assert_eq!(choose("Ava veebilehitseja", &TargetRef::None).0, "app");
    // The head of the main clause decides when both are named: a game used as the source or
    // topic is not the build.
    for request in ["Tee sellest mängust veebileht", "Mängude veebileht", "Loo veebileht, kus müüakse mänge", "Veebileht mängust"] {
        assert_eq!(choose(request, &TargetRef::None).0, "website", "{request}");
    }
    for request in ["Tee sellest veebilehest mäng", "Loo mäng nimega Veebileht", "Mäng veebilehega"] {
        assert_eq!(choose(request, &TargetRef::None).0, "game", "{request}");
    }
}

#[test]
fn chinese_requests_keep_their_project_kind() {
    for request in ["一个简单的贪吃蛇游戏", "簡單的貪食蛇遊戲", "复刻它的玩法"] {
        assert_eq!(choose(request, &TargetRef::None), ("game", "Unity"), "{request}");
    }
    for request in ["给面包店做一个网站", "為我的麵包店製作網站", "做一个网页", "製作網頁"] {
        assert_eq!(choose(request, &TargetRef::None), ("website", stacks::BUILDERS_CHOICE), "{request}");
    }
    assert_eq!(choose("制作一个笔记应用", &TargetRef::None).0, "app");
}

#[test]
fn site_dir_prefers_build_output() {
    let d = tempfile::tempdir().unwrap();
    assert!(site_dir(d.path()).is_none());
    std::fs::write(d.path().join("index.html"), "x").unwrap();
    assert_eq!(site_dir(d.path()).as_deref(), Some(d.path()), "a plain static site");
    std::fs::write(d.path().join("package.json"), "{}").unwrap();
    assert!(site_dir(d.path()).is_none(), "a source index.html of an unbuilt Vite app doesn't count");
    std::fs::create_dir(d.path().join("dist")).unwrap();
    std::fs::write(d.path().join("dist/index.html"), "x").unwrap();
    assert_eq!(site_dir(d.path()), Some(d.path().join("dist")));
}

#[test]
fn built_program_finds_the_cargo_output_only() {
    let d = tempfile::tempdir().unwrap();
    let debug = d.path().join("target/debug");
    std::fs::create_dir_all(debug.join("build")).unwrap();
    let exe = |name: &str| if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let make = |p: &Path| {
        std::fs::write(p, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    };
    make(&debug.join(exe("snake")));
    std::fs::write(debug.join("snake.d"), "deps").unwrap();
    make(&debug.join("build").join(exe("build-script-build")));
    assert!(built_program(d.path()).is_none(), "not a cargo project");
    std::fs::write(d.path().join("Cargo.toml"), "[package]").unwrap();
    assert_eq!(built_program(d.path()), Some(debug.join(exe("snake"))));
}

#[test]
fn play_starts_the_window_app_of_a_workspace_not_its_newest_program() {
    // Play started the CLI a workspace also built (it was newer), which printed help and quit
    // with no console, so nothing appeared.
    let d = tempfile::tempdir().unwrap();
    let app = d.path();
    let debug = app.join("target/debug");
    std::fs::create_dir_all(&debug).unwrap();
    std::fs::create_dir_all(app.join("src")).unwrap();
    std::fs::create_dir_all(app.join("crates/cli/src")).unwrap();
    std::fs::create_dir_all(app.join("crates/doc/src")).unwrap();
    std::fs::write(app.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(app.join("crates/cli/src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(
        app.join("Cargo.toml"),
        "[workspace]\nmembers = [\".\", \"crates/*\"]\n\n[package]\nname = \"anim\"\n\n[dependencies]\neframe = { workspace = true }\n",
    )
    .unwrap();
    std::fs::write(app.join("crates/cli/Cargo.toml"), "[package]\nname = \"anim-cli\"\n\n[[bin]]\nname = \"anim-cli\"\npath = \"src/main.rs\"\n").unwrap();
    std::fs::write(app.join("crates/doc/Cargo.toml"), "[package]\nname = \"anim-doc\"\n").unwrap();
    let exe = |name: &str| if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let make = |name: &str| {
        let p = debug.join(exe(name));
        std::fs::write(&p, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Each later one is newer.
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    make("anim");
    make("anim.running-144900");
    make("anim-cli");
    assert_eq!(built_program(app), Some(debug.join(exe("anim"))), "the package that opens a window");
    // A default-run says which, whatever it uses.
    std::fs::write(
        app.join("Cargo.toml"),
        "[workspace]\nmembers = [\".\", \"crates/*\"]\n\n[package]\nname = \"anim\"\ndefault-run = \"anim-cli\"\n\n[dependencies]\neframe = \"0.36\"\n",
    )
    .unwrap();
    assert_eq!(built_program(app), Some(debug.join(exe("anim-cli"))));
}

#[test]
fn a_launcher_leads_to_the_game_and_its_files() {
    let d = tempfile::tempdir().unwrap();
    let game = d.path();
    for dir in ["bin_win32", "packages/maps", "packages/textures/walls", "packages/audio"] {
        std::fs::create_dir_all(game.join(dir)).unwrap();
    }
    let exe = |n: &str| if cfg!(windows) { format!("{n}.exe") } else { n.to_string() };
    let make = |p: &Path, bytes: usize| {
        std::fs::write(p, vec![0u8; bytes]).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    };
    make(&game.join("bin_win32").join(exe("client")), 10);
    make(&game.join("bin_win32").join(exe("server")), 99); // bigger, but a server isn't the game
    make(&game.join(exe("Uninstall")), 500);
    std::fs::write(game.join("packages/maps/dust.cgz"), "m").unwrap();
    std::fs::write(game.join("packages/textures/walls/brick.jpg"), "t").unwrap();
    std::fs::write(game.join("packages/audio/shot.ogg"), "s").unwrap();
    std::fs::write(game.join("packages/textures/walls/license.txt"), "CC-BY").unwrap();
    let bat = game.join("play.bat");
    std::fs::write(&bat, format!("start bin_win32\\{} \"--home=x\" --init %1\n", exe("client"))).unwrap();

    let (program, folder) = locate(&bat, "app");
    assert_eq!(program, game.join("bin_win32").join(exe("client")), "the program the script starts");
    assert_eq!(folder.as_deref(), Some(game), "a launcher's folder is studied whatever the kind");
    assert_eq!(main_program(game), Some(game.join("bin_win32").join(exe("client"))), "skips installers and servers");
    let (_, up) = locate(&game.join("bin_win32").join(exe("client")), "game");
    // Picking Unreal's shared Engine/ folder (or Binaries/Win64) finds the game above it.
    let ue = d.path().join("Untitled Project");
    let shipping = ue.join("MyGame/Binaries/Win64").join(exe("MyGame-Win64-Shipping"));
    std::fs::create_dir_all(shipping.parent().unwrap()).unwrap();
    make(&shipping, 4096);
    std::fs::create_dir_all(ue.join("Engine/Binaries/Win64")).unwrap();
    std::fs::write(ue.join("Engine/Binaries/Win64").join(exe("CrashReportClient")), b"MZ").unwrap();
    for picked in [ue.join("Engine"), ue.clone()] {
        assert_eq!(locate(&picked, "game"), (shipping.clone(), Some(ue.clone())), "{}", picked.display());
    }
    // Inside the game's own project folder, that folder (with its Content/) is the game.
    assert_eq!(locate(&ue.join("MyGame").join("Binaries"), "game"), (shipping.clone(), Some(ue.join("MyGame"))));
    assert_eq!(game_root(d.path()), d.path(), "a folder that isn't inside a game stays as picked");
    assert_eq!(up.as_deref(), Some(game), "content lives above bin_win32");

    let inv = inventory(game);
    assert!(inv.contains("Maps and levels: 1 files") && inv.contains("Textures and pictures: 1 files") && inv.contains("Sounds and music: 1 files"), "{inv}");
    assert!(inv.contains("brick.jpg"));
    assert!(inv.contains("Read-me and licence files: 1 files") && !inv.contains("Settings and scripts"), "{inv}");
}

#[test]
fn a_mac_app_bundle_is_studied_through_the_program_it_names() {
    let d = tempfile::tempdir().unwrap();
    // The executable's name differs from the bundle's, and a bigger helper sits beside it.
    let app = d.path().join("Tip Calculator.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>TipCalc</string></dict></plist>"#,
    )
    .unwrap();
    std::fs::write(macos.join("TipCalc"), b"\xcf\xfa\xed\xfe").unwrap();
    std::fs::write(macos.join("Helper"), vec![0u8; 4096]).unwrap();
    assert_eq!(locate(&app, "app"), (macos.join("TipCalc"), None), "the program Info.plist names");

    // Without an Info.plist, the program named like the bundle.
    let bare = d.path().join("Notes.APP");
    std::fs::create_dir_all(bare.join("Contents/MacOS")).unwrap();
    std::fs::write(bare.join("Contents/MacOS/Notes"), b"\xcf\xfa\xed\xfe").unwrap();
    assert_eq!(locate(&bare, "app").0, bare.join("Contents/MacOS/Notes"));

    // A bundle with no program in it, and a folder that isn't a bundle, stay as picked.
    let empty = d.path().join("Empty.app");
    std::fs::create_dir_all(empty.join("Contents/MacOS")).unwrap();
    assert_eq!(locate(&empty, "app"), (empty.clone(), None));
    let plain = d.path().join("TipCalc");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("TipCalc"), b"\xcf\xfa\xed\xfe").unwrap();
    assert_eq!(locate(&plain, "app"), (plain.clone(), None), "only an app bundle is looked inside");
}

#[test]
fn engine_stacks_get_their_own_steps_checks_and_play() {
    let ed = Path::new("C:/Engines/Unity/Editor/Unity.exe");
    let unity = stack_steps("Unity", Some(ed));
    assert!(unity.contains("CraftBuild.Build") && unity.contains("Unity.exe") && unity.contains("app/Build/Game."));
    assert!(stack_steps("Godot", Some(ed)).contains("--headless --path app"));
    assert!(stack_steps("Unreal Engine", Some(ed)).contains("Game.uproject"));

    let d = tempfile::tempdir().unwrap();
    let stop = CheckStop::default();
    assert_eq!(build_check(d.path(), "app/", "Unity", Some(ed), &stop), (false, "No Unity project found in app/ yet.".into()));
    assert_eq!(build_check(d.path(), "app/", "Godot", Some(ed), &stop), (false, "No Godot project (project.godot) found in app/ yet.".into()));
    assert!(build_check(d.path(), "app/", "Unreal Engine", Some(ed), &stop).1.contains(".uproject"));
    // #174: an app edited in place is said to be in the project folder, not in an app/ that isn't there.
    let none = build_check(d.path(), "the project folder", "Builder's choice", None, &stop);
    assert_eq!(none, (false, "No project to build found in the project folder yet.".into()));

    // Play: a Godot project opens in Godot; a packaged Unity build runs its own program.
    std::fs::write(d.path().join("project.godot"), "config_version=5").unwrap();
    let g = Path::new("C:/godot.exe");
    assert_eq!(launch(d.path(), Some(g)), Some((g.to_path_buf(), vec!["--path".into(), d.path().display().to_string()])));
    assert_eq!(launch(d.path(), None), None, "no Godot, nothing to open it with");
    std::fs::remove_file(d.path().join("project.godot")).unwrap();
    let build = d.path().join("Build");
    std::fs::create_dir_all(&build).unwrap();
    let exe = |n: &str| if cfg!(windows) { format!("{n}.exe") } else { n.to_string() };
    for n in ["Game", "UnityCrashHandler64"] {
        let p = build.join(exe(n));
        std::fs::write(&p, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    assert_eq!(launch(d.path(), None).map(|l| l.0), Some(build.join(exe("Game"))), "crash reporters aren't the game");
}

#[test]
fn games_hear_about_generated_assets_only_when_allowed_and_with_a_key() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Game", "A snake game", TargetRef::None).unwrap();
    ws.manifest.stack = "Unity".into();
    assert!(!brief(&ws, "game", false, false, None, true, None).contains("assets_model_generate"), "a key alone isn't the person's choice");
    ws.manifest.generate_assets = true;
    let with = brief(&ws, "game", false, false, None, true, None);
    assert!(with.contains("assets_model_generate") && with.contains("$5.00") && with.contains("Assets/Generated/"), "{with}");
    assert!(with.contains("never a recognisable character, logo or brand artwork"));
    assert!(!brief(&ws, "game", false, false, None, false, None).contains("assets_model_generate"), "no key, no asset tools");
    assert!(!brief(&ws, "app", false, false, None, true, None).contains("assets_model_generate"), "games only");
}

#[test]
fn a_new_project_keeps_the_persons_asset_choice() {
    let d = tempfile::tempdir().unwrap();
    let settings = craft_anything_engine::Settings::default();
    for allow in [false, true] {
        let ws = project::new_project(&settings, "-", "A snake game", project::Place::In(d.path()), Some("unity"), allow, None).unwrap();
        assert_eq!(Workspace::open(&ws.root).unwrap().manifest.generate_assets, allow);
    }
}

#[test]
fn the_assets_note_reads_prices_from_the_asset_table() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Game", "A snake game", TargetRef::None).unwrap();
    // The default picture model is Nano Banana Pro ($0.15 a picture).
    let before = "Realistic assets: you can make them with fal.ai. assets_model_generate makes a textured, game-ready low-poly 3D model (GLB) of one object from a description (about $0.65 each); set faces to the fewest that still look right (about 500-1500 for small props, 2000-5000 for weapons and vehicles, 6000-15000 for main characters) so the game stays fast; assets_image_generate makes a picture (about $0.15) for textures, skies, backgrounds and menu art. This project's asset budget is $5.00 ($0.00 used), so spend it on what the player sees most (the main characters, weapons, vehicles and the level's key props) and use simple shapes for the rest. Save each one inside app/ where the engine loads it (e.g. assets/generated/). Every asset is an original design in the original's style: describe what it is, its shape, materials and colours, never a recognisable character, logo or brand artwork. List each generated file in app/CREDITS.md as made with fal.ai.";
    assert_eq!(assets_note(&ws, None), before);
    assert_eq!(assets_note(&ws, Some("no-such-model")), before, "an unknown model quotes the default's prices");
    let schnell = assets_note(&ws, Some("flux-schnell"));
    assert!(schnell.contains("(about $0.50 each)") && schnell.contains("a picture (under $0.01)"), "{schnell}");
}

#[test]
fn notes_added_while_it_works_reach_the_agent_at_its_next_step() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "x", "Copy this game", TargetRef::None).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let notes: Arc<Mutex<Vec<Note>>> = Arc::default();
    let w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: notes.clone(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    assert!(w.added(&mut ws, true, true).is_none(), "nothing added, nothing to say");
    use craft_anything_engine::questions::{self, AskedQuestion};
    let asked = AskedQuestion { deep: false, ..AskedQuestion::new("where is the score kept".into(), "In src.".into(), true, "Stand-in".into(), None, 0) };
    questions::add(&ws, asked.clone()).unwrap();
    let note = |id, text: &str| Note { id, text: format!("{text} (pictures: a.png)"), shown: text.into(), pictures: Vec::new(), questions: Vec::new() };
    let mut first = note(1, "Make it faster");
    first.questions = vec![asked];
    notes.lock().unwrap().extend([first, note(2, "A blue sky")]);
    let (p, more) = w.added(&mut ws, true, true).unwrap();
    assert!(p.contains("where is the score kept") && p.contains("- Make it faster (pictures: a.png)\n- A blue sky"), "{p}");
    assert_eq!(more.len(), 1, "the question is marked told after the turn");
    assert!(notes.lock().unwrap().is_empty(), "each note goes in once");
    // The notes join the feed as they're read, before the step that reads them.
    let events: Vec<RunEvent> = rx.try_iter().collect();
    let shown: Vec<_> = events.iter().filter_map(|e| if let RunEvent::Note { shown, .. } = e { Some(shown.as_str()) } else { None }).collect();
    assert_eq!(shown, ["Make it faster", "A blue sky"]);
    assert!(matches!(events.last(), Some(RunEvent::Step { text, .. }) if text == "Reading what you added…"));
}

/// #857: a note handed over by a coding agent's tool hook never sees the after-turn message, so
/// the Just-ask answers are put in the note itself (only the ones this mode may show).
#[test]
fn a_note_carries_the_just_ask_answers_once_and_only_in_their_mode() {
    use craft_anything_engine::questions::{self, AskedQuestion};
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
    let clean =
        AskedQuestion { deep: false, ..AskedQuestion::new("where is the score kept".into(), "In src/score.rs.".into(), true, "Stand-in".into(), None, 0) };
    questions::add(&ws, clean.clone()).unwrap();
    let (text, carried) = note_with_just_asked("make it blue".into(), &ws, false, &[]);
    assert!(text.contains("where is the score kept") && text.contains("make it blue"), "{text}");
    assert_eq!(carried.len(), 1);

    // A note already waiting carries it; the next one doesn't repeat it.
    let (again, none) = note_with_just_asked("make it green".into(), &ws, false, &carried);
    assert!(!again.contains("where is the score kept") && none.is_empty(), "{again}");

    // A Deep question stays out of a Clean room note.
    questions::add(
        &ws,
        AskedQuestion { deep: true, ..AskedQuestion::new("what does the original do".into(), "It reads a file.".into(), true, "Stand-in".into(), None, 0) },
    )
    .unwrap();
    let (clean_note, carried) = note_with_just_asked("make it blue".into(), &ws, true, &[]);
    assert!(clean_note.contains("where is the score kept") && !clean_note.contains("what does the original do"), "{clean_note}");
    assert_eq!(carried.len(), 1, "only the Clean room question");
}

/// The app writes the enriched note for both hand-over paths, so the hook prints the answers.
#[test]
fn a_note_written_for_the_hook_carries_its_answers() {
    use craft_anything_engine::questions::{self, AskedQuestion};
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
    let asked =
        AskedQuestion { deep: false, ..AskedQuestion::new("where is the score kept".into(), "In src/score.rs.".into(), true, "Stand-in".into(), None, 0) };
    questions::add(&ws, asked.clone()).unwrap();
    let (_tx, rx) = std::sync::mpsc::channel();
    let feed = Arc::new(Mutex::new(feed::FeedLog::open(&ws)));
    let activity = feed.lock().unwrap_or_else(PoisonError::into_inner).activity();
    let run = RebuildRun {
        rx,
        stop: CancelToken::default(),
        inbox: Inbox::new(ws.inbox_dir()),
        running: Arc::new(AtomicBool::new(true)),
        preview: Arc::default(),
        notes: Arc::default(),
        clean_room: false,
        feed,
        root: ws.root.clone(),
        started: Instant::now(),
        activity,
    };
    let id = run.add_note("make it blue".into(), "make it blue".into(), Vec::new());
    let note = run.waiting().pop().unwrap();
    assert_eq!(note.text, "make it blue", "the in-memory text stays raw for a later run's mode");
    assert_eq!(note.questions.len(), 1);

    let dir = notes::notes_dir(&ws.inbox_dir());
    let claimed = notes::claim(&dir);
    assert_eq!(claimed.len(), 1);
    assert!(claimed[0].contains("where is the score kept"), "{}", claimed[0]);
    assert!(notes::was_read(&dir, id));
    // The run ended before relaying it: its raw text goes next, and the next run's mode decides
    // whether the answers are included, so they stay untold here.
    assert_eq!(run.take_notes().len(), 1);
    assert_eq!(questions::untold(&ws, false).len(), 1, "the next run decides whether to tell it");
}

#[test]
fn pictures_of_the_original_reach_the_builder() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Game", "Copy this game", TargetRef::None).unwrap();
    assert!(!brief(&ws, "game", false, false, None, false, None).contains("reference/picture"));
    assert!(pictures_reminder(&ws, true).is_empty());
    let pic = d.path().join("shot.webp");
    std::fs::write(&pic, "x").unwrap();
    ws.add_references(&[pic]).unwrap();
    let b = brief(&ws, "game", false, false, None, false, None);
    assert!(b.contains("reference/picture-1.webp") && b.contains("--screenshot"), "{b}");
    assert!(!brief(&ws, "website", false, false, None, false, None).contains("--screenshot"), "websites are photographed for it");
    assert!(pictures_reminder(&ws, true).contains("1 picture(s)"));
}

/// What a builder that can't look at pictures (the API loop) is told to open or look at a
/// picture with. A sighted builder's prompts say these; a blind one's never do (R5.4).
const LOOK_AT_A_PICTURE: &[&str] = &[
    "Read tool and look",
    "with the Read tool to learn the style",
    "Open every one with the Read tool",
    "open that picture with Read",
    "Open each pair",
    "open any you haven't looked at",
    // The --screenshot contract's old wording, kept here so it can't come back for a blind builder.
    "now.png) and look at the picture",
    "open it with Read and look at it",
    "from the pictures and how it plays",
    "use the pictures in reference/",
    "Work from these pictures",
    "so match how they look",
];

fn said_to_look(text: &str) -> Option<&'static str> {
    LOOK_AT_A_PICTURE.iter().copied().find(|p| text.contains(p))
}

/// R5.4: every first message, refinement and reminder a blind builder can get, for every kind,
/// target, mode and with or without the person's pictures, sends it to no picture, and each says
/// what to use instead.
#[test]
fn a_builder_that_cant_see_is_never_sent_to_a_picture() {
    let sees_only = |ws: &Workspace, k: &str, deep: bool, assets: bool, sees_pictures: bool| {
        super::brief(kind(k).as_ref(), &BriefCx { ws, deep, ghidra: false, editor: None, assets, image_model: None, sees_pictures }, None)
    };
    let mut checked = 0;
    // Everything a sighted builder is sent, to show the guard's phrases are still what it's told.
    let mut sighted_all = String::new();
    for k in KindRegistry::built_in().ids() {
        for target in ["none", "web", "local"] {
            for (deep, assets, pics, generated) in (0..16).map(|b| (b & 1 != 0, b & 2 != 0, b & 4 != 0, b & 8 != 0)) {
                let d = tempfile::tempdir().unwrap();
                let t = match target {
                    "web" => TargetRef::Website { url: "https://example.com".into() },
                    "local" => TargetRef::Local { path: d.path().join("orig").join("game.exe") },
                    _ => TargetRef::None,
                };
                let mut ws = Workspace::create(d.path(), "P", "Make it", t).unwrap();
                ws.manifest.generate_assets = generated;
                if pics {
                    let p = d.path().join("shot.png");
                    std::fs::write(&p, "x").unwrap();
                    ws.add_references(&[p]).unwrap();
                }
                let case = format!("{k} {target} deep={deep} assets={assets} pictures={pics} generated={generated}");
                let blind = sees_only(&ws, k, deep, assets, false);
                assert_eq!(said_to_look(&blind), None, "{case}:\n{blind}");
                sighted_all.push_str(&sees_only(&ws, k, deep, assets, true));
                sighted_all.push_str(&pictures_reminder(&ws, true));
                if pics {
                    assert!(blind.contains("you can't look at pictures") && blind.contains("core_person_ask"), "{case}:\n{blind}");
                    assert_eq!(said_to_look(&pictures_reminder(&ws, false)), None, "{case}");
                    assert!(pictures_reminder(&ws, false).contains("you can't look at them"), "{case}");
                }
                if target == "web" && k == "website" {
                    assert!(blind.contains("`layouts`") && !blind.contains("screenshot and `layouts`"), "{case}:\n{blind}");
                }
                if target == "local" && !deep {
                    assert!(blind.contains("capture/ui-tree.json"), "{case}:\n{blind}");
                }
                checked += 1;
            }
        }
        let mut c = CheckResult::new("x", 3, true, String::from("the note"));
        assert_eq!(said_to_look(&kind(k).refine(&c, false)), None, "{k}");
        c.similarity = Some(71.0);
        let blind = kind(k).refine(&c, false);
        assert_eq!(said_to_look(&blind), None, "{k}: {blind}");
        if k == "website" {
            assert!(blind.contains("checks/round-3/capture.json") && blind.contains("capture/capture.json"), "{blind}");
        }
        sighted_all.push_str(&kind(k).refine(&c, true));
    }
    assert!(checked >= 100, "only {checked} cases");
    sighted_all.push_str(&super::kinds::screenshot::contract(true));
    for phrase in LOOK_AT_A_PICTURE.iter().filter(|p| **p != "now.png) and look at the picture") {
        assert!(sighted_all.contains(phrase), "{phrase:?} is no longer said to any builder: the guard is stale");
    }
    // The --screenshot contract, also sent inside a failed check's note.
    assert_eq!(said_to_look(&super::kinds::screenshot::contract(false)), None);
    assert_eq!(said_to_look(super::kinds::screenshot::SCREENSHOT_CONTRACT), None, "a check's note may reach a blind builder");
    // The API loop's own note says it once; agent apps aren't told.
    assert!(!API_BUILDER.contains("can't look at pictures") && BLIND_BUILDER.contains("can't look at pictures"));
}

mod typed_retries {
    use super::*;
    use crate::{FailureKind, TurnFailure};
    use std::collections::VecDeque;

    struct FailingConversation {
        failures: VecDeque<Option<TurnFailure>>,
        failure: Option<TurnFailure>,
        queued: Vec<AgentEvent>,
        sent: Vec<String>,
        session: Option<String>,
        message: &'static str,
    }

    impl FailingConversation {
        fn new(failures: impl IntoIterator<Item = Option<TurnFailure>>, session: bool) -> Self {
            Self {
                failures: failures.into_iter().collect(),
                failure: None,
                queued: Vec::new(),
                sent: Vec::new(),
                session: session.then(|| "saved".into()),
                message: "provider response",
            }
        }
    }

    impl Conversation for FailingConversation {
        fn send(&mut self, text: &str) {
            self.sent.push(text.into());
            self.failure = self.failures.pop_front().flatten();
            self.queued.push(if self.failure.is_some() {
                AgentEvent::Error { message: self.message.into(), remediation: None, log: None }
            } else {
                AgentEvent::TextDelta { text: "Done".into() }
            });
            self.queued.push(AgentEvent::TurnDone { backend: "scripted".into(), input_tokens: 0, output_tokens: 0, cost_usd: None, cost_is_estimate: false });
        }
        fn session_id(&self) -> Option<String> {
            self.session.clone()
        }
        fn last_failure(&self) -> Option<TurnFailure> {
            self.failure
        }
        fn poll(&mut self) -> Vec<AgentEvent> {
            std::mem::take(&mut self.queued)
        }
        fn answer_consent(&mut self, _: &str, _: bool) {}
        fn stop(&mut self) {}
        fn is_busy(&self) -> bool {
            false
        }
    }

    /// A retryable CLI failure is retried without asking for a fresh external session.
    #[test]
    fn transient_failures_retry_without_requiring_an_external_session() {
        for saved in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
            let mut worker = check_worker(&ws, false);
            let failure = TurnFailure { kind: FailureKind::RateLimited, retry_after_secs: Some(2) };
            let mut agent = FailingConversation::new([Some(failure), None], saved);
            agent.message = "Authentication and billing service restarting. Please retry in 90s.";
            let mut waits = Vec::new();
            let result = worker.turn_with_wait(&mut ws, &mut agent, "Build the requested app", true, None, |_, seconds| {
                waits.push(seconds);
                Ok(())
            });
            assert_eq!(result.unwrap(), "Done");
            assert_eq!(waits, [3], "typed provider wait overrides the text hint");
            assert_eq!(agent.sent.len(), 2);
            assert_eq!(agent.sent[0], "Build the requested app");
            assert_eq!(agent.sent[1], if saved { crate::failure::CONTINUE_TURN } else { "Build the requested app" });
        }
    }

    /// Typed setup errors stop at once, even when their text contains a retry hint.
    #[test]
    fn setup_errors_do_not_retry_even_with_a_wait_in_the_message() {
        for kind in [
            FailureKind::Authentication,
            FailureKind::Billing,
            FailureKind::MissingProgram,
            FailureKind::ToolsUnavailable,
            FailureKind::Configuration,
            FailureKind::Unknown,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
            let mut worker = check_worker(&ws, false);
            let mut agent = FailingConversation::new([Some(TurnFailure::new(kind))], true);
            agent.message = "需要修改设置。Please retry in 1s.";
            let result = worker.turn_with_wait(&mut ws, &mut agent, "Build it", true, None, |_, _| panic!("a setup error must not wait and retry"));
            assert!(result.is_err(), "{kind:?}");
            assert_eq!(agent.sent.len(), 1, "{kind:?}");
        }
    }

    /// Retries stop at the cap, and a cancelled wait sends no further turn.
    #[test]
    fn retries_are_bounded_and_cancelled_waits_send_nothing_more() {
        assert_eq!(RunOptions::default().max_rounds, 6, "room to keep fixing until it works");
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let mut worker = check_worker(&ws, false);
        let failure = TurnFailure::new(FailureKind::Transient);
        let mut agent = FailingConversation::new([Some(failure); 5], false);
        let mut waits = Vec::new();
        assert!(
            worker
                .turn_with_wait(&mut ws, &mut agent, "Build it", true, None, |_, seconds| {
                    waits.push(seconds);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(agent.sent.len(), 4, "initial attempt and at most three retries");
        assert_eq!(waits, [30, 60, 90]);
        let mut agent = FailingConversation::new([Some(failure)], false);
        let error = worker
            .turn_with_wait(&mut ws, &mut agent, "Build it", true, None, |stop, _| {
                stop.cancel();
                stop.check()
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert_eq!(agent.sent.len(), 1);
    }

    #[test]
    fn a_local_self_check_write_error_is_not_retried_as_a_backend_rate_limit() {
        #[derive(Clone)]
        struct BlockReply {
            answer: PathBuf,
        }
        impl Check for BlockReply {
            fn id(&self) -> &'static str {
                "blocked-reply"
            }
            fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
                // The request was pending when the run read it; its answer now cannot be
                // written. The workspace itself remains writable, including retry status.
                std::fs::create_dir_all(&self.answer).unwrap();
                Ok(CheckResult::new(Check::id(self), cx.round, true, "The check finished."))
            }
        }
        impl TargetKind for BlockReply {
            fn id(&self) -> &'static str {
                "blocked-reply"
            }
            fn words(&self) -> KindWords {
                SlidesKind.words()
            }
            fn detect(&self, _: &DetectCx) -> Option<u32> {
                Some(1)
            }
            fn default_stack(&self, cx: &DetectCx) -> &'static str {
                SlidesKind.default_stack(cx)
            }
            fn goal(&self) -> &'static str {
                SlidesKind.goal()
            }
            fn checks(&self, _: &Workspace) -> Vec<Box<dyn Check>> {
                vec![Box::new(self.clone())]
            }
        }
        struct RateLimited {
            sent: usize,
            first_poll: bool,
        }
        impl Conversation for RateLimited {
            fn send(&mut self, _: &str) {
                self.sent += 1;
                self.first_poll = true;
            }
            fn poll(&mut self) -> Vec<AgentEvent> {
                vec![if std::mem::take(&mut self.first_poll) {
                    AgentEvent::Error { message: "The provider is rate limited.".into(), remediation: None, log: None }
                } else {
                    AgentEvent::TurnDone { backend: "scripted".into(), input_tokens: 0, output_tokens: 0, cost_usd: None, cost_is_estimate: false }
                }]
            }
            fn last_failure(&self) -> Option<TurnFailure> {
                Some(TurnFailure::new(FailureKind::RateLimited))
            }
            fn answer_consent(&mut self, _: &str, _: bool) {}
            fn stop(&mut self) {}
            fn is_busy(&self) -> bool {
                false
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let asks = Inbox::new(self_check::asks_dir(&ws.root));
        let id = asks.post(craft_anything_engine::inbox::RequestKind::Question, "Check the build", "", Vec::new(), false, None).unwrap();
        let mut worker = check_worker(&ws, false);
        worker.self_check = Some((Arc::new(BlockReply { answer: asks.dir().join(format!("{id}.answer.json")) }), 90.0));
        let mut agent = RateLimited { sent: 0, first_poll: false };
        let mut waits = Vec::new();
        let error = worker
            .turn_with_wait(&mut ws, &mut agent, "Build the app", true, None, |_, seconds| {
                waits.push(seconds);
                Ok(())
            })
            .unwrap_err();
        assert_eq!(agent.sent, 1, "a backend's retry hint cannot retry a local persistence failure");
        assert!(waits.is_empty());
        assert_ne!(error.code, ErrorCode::ProgramFailed);
        assert!(error.message.contains(".answer.json"), "the original local failure is retained: {error:?}");
    }

    /// A rate-limited API turn continues its history without repeating the person's message or
    /// completed tools.
    #[test]
    fn an_api_rate_limit_continues_history_without_repeating_users_or_completed_tools() {
        use crate::agent::tests::{Scripted, turn};
        use crate::agent::{ApiConversation, EngineHost, LoopConfig, ModelSlot};
        use crate::model::{Block, Speaker, StopReason};
        for write_first in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut ws = Workspace::create(dir.path(), "x", "Build a notes app", TargetRef::None).unwrap();
            let mut worker = check_worker(&ws, false);
            worker.attach(&ws).unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            worker.tx = tx;
            let seen = Arc::default();
            let mut script = Scripted { turns: VecDeque::new(), seen: Arc::clone(&seen) };
            if write_first {
                script.turns.push_back(turn(
                    vec![Block::ToolUse {
                        id: "write-once".into(),
                        name: "build_file_write".into(),
                        input: json!({"path":"app/README.md","content":"Built once"}),
                    }],
                    StopReason::ToolUse,
                ));
            }
            script.turns.push_back(Err(ToolError::new(ErrorCode::OverLimit, "服务限流").with_details(json!({"status":429,"retry_after_secs":3}))));
            script.turns.push_back(turn(vec![Block::Text { text: "Built the notes app".into() }], StopReason::EndTurn));
            let slot = ModelSlot {
                name: "Scripted".into(),
                model: Box::new(script),
                local: false,
                effort: None,
                max_turns: 10,
                context_tokens: 0,
                tools: Default::default(),
            };
            let enabled = std::collections::BTreeSet::from([craft_anything_engine::Toolset::Core, craft_anything_engine::Toolset::Build]);
            let mut api = ApiConversation::start_with(Arc::new(EngineHost(worker.engine.clone())), vec![slot], LoopConfig::default(), enabled).unwrap();
            let mut waits = Vec::new();
            assert_eq!(
                worker
                    .turn_with_wait(&mut ws, &mut api, "Build the notes app", true, None, |_, seconds| {
                        waits.push(seconds);
                        Ok(())
                    })
                    .unwrap(),
                "Built the notes app"
            );
            assert_eq!(waits, [4]);
            assert!(ws.manifest.agent_session.is_none());
            let requests = seen.lock().unwrap();
            assert_eq!(requests.len(), if write_first { 3 } else { 2 });
            let before_retry = &requests[requests.len() - 2];
            let after_retry = requests.last().unwrap();
            assert_eq!(before_retry, after_retry, "retry must continue the same model request");
            assert_eq!(after_retry.iter().filter(|m| m.role == Speaker::User && m.text() == "Build the notes app").count(), 1);
            let calls = rx.try_iter().filter(|e| matches!(e, RunEvent::Agent(AgentEvent::ToolStarted { tool, .. }) if tool == "build_file_write")).count();
            assert_eq!(calls, usize::from(write_first), "a completed write is not replayed");
            if write_first {
                assert_eq!(std::fs::read_to_string(ws.app_dir().join("README.md")).unwrap(), "Built once");
            }
        }
    }

    /// Write an executable stand-in CLI and start a conversation with it.
    #[cfg(unix)]
    fn executable_cli(ws: &Workspace, model: &str, script: &str) -> crate::cli_agents::CliConversation {
        use std::os::unix::fs::PermissionsExt;
        let program = ws.root.join("agent-stand-in.sh");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = crate::BackendConfig {
            name: "Scripted CLI".into(),
            kind: BackendKind::AgentCli,
            model: model.into(),
            base_url: None,
            ssh_host: None,
            enabled: true,
        };
        let options = crate::BackendOptions { command: Some(program.display().to_string()), timeout_secs: Some(10), ..Default::default() };
        let mut spec = CliSpec::new(config, options, false, None);
        spec.project = Some(ws.root.clone());
        spec.preamble = Some(String::new());
        crate::cli_agents::CliConversation::new(spec)
    }

    #[cfg(unix)]
    /// A failed required tool server stops the owned CLI instead of resuming its session.
    #[test]
    fn a_failed_required_tool_server_stops_the_owned_cli_without_resuming_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let mut worker = check_worker(&ws, false);
        let (tx, rx) = std::sync::mpsc::channel();
        worker.tx = tx;
        let mut cli = executable_cli(
            &ws,
            "claude-code",
            concat!(
                "#!/bin/sh\ncat >/dev/null\nprintf 'attempt\\n' >> attempts\n",
                "printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"cannot-build\",\"mcp_servers\":[{\"name\":\"craft-anything\",\"status\":\"failed\"}]}'\n",
                "sleep 5\nprintf 'unreachable' > continued-without-tools\n"
            ),
        );
        let error = worker.turn_with_wait(&mut ws, &mut cli, "Build the app", true, None, |_, _| panic!("missing tools must not be retried")).unwrap_err();
        assert!(error.message.contains("tools did not start"), "{error:?}");
        assert_eq!(cli.last_failure().unwrap().kind, FailureKind::ToolsUnavailable);
        assert!(!cli.is_busy());
        assert_eq!(std::fs::read_to_string(ws.root.join("attempts")).unwrap(), "attempt\n");
        assert!(!ws.root.join("continued-without-tools").exists(), "the owned child was stopped at the failed init");
        assert_eq!(rx.try_iter().filter(|e| matches!(e, RunEvent::Agent(AgentEvent::TurnDone { .. }))).count(), 1);
    }

    #[cfg(unix)]
    /// A plain CLI's non-zero exit is retried by resuming the session it saved.
    #[test]
    fn a_plain_nonzero_cli_exit_resumes_its_saved_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let mut worker = check_worker(&ws, false);
        let (tx, rx) = std::sync::mpsc::channel();
        worker.tx = tx;
        let mut cli = executable_cli(
            &ws,
            "claude-code",
            concat!(
                "#!/bin/sh\ncat >> prompts\nprintf '\\n' >> prompts\nprintf 'attempt\\n' >> attempts\n",
                "case \" $* \" in *\" --resume interrupted-session \"*)\n",
                "printf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Built after the interruption\"}]}}'\n",
                "printf '%s\\n' '{\"type\":\"result\",\"is_error\":false,\"result\":\"Built after the interruption\"}'\nexit 0;; esac\n",
                "printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"interrupted-session\"}'\nexit 1\n"
            ),
        );
        let mut waits = Vec::new();
        let result = worker.turn_with_wait(&mut ws, &mut cli, "Make a notes app", true, None, |_, seconds| {
            waits.push(seconds);
            Ok(())
        });
        assert_eq!(std::fs::read_to_string(ws.root.join("attempts")).unwrap(), "attempt\nattempt\n", "a plain nonzero exit must resume the reported session");
        assert_eq!(result.unwrap(), "Built after the interruption");
        assert_eq!(waits, [30]);
        assert_eq!(std::fs::read_to_string(ws.root.join("prompts")).unwrap(), format!("Make a notes app\n{}\n", crate::failure::CONTINUE_TURN));
        assert_eq!(cli.session_id().as_deref(), Some("interrupted-session"));
        assert!(cli.last_failure().is_none(), "successful recovery clears the process failure");
        assert!(!cli.is_busy());
        assert_eq!(rx.try_iter().filter(|e| matches!(e, RunEvent::Agent(AgentEvent::TurnDone { .. }))).count(), 2);
    }

    #[cfg(unix)]
    /// A typed setup error is reported ahead of a non-zero exit.
    #[test]
    fn typed_cli_setup_errors_take_precedence_over_a_nonzero_exit() {
        for (code, kind) in
            [("authentication_failed", FailureKind::Authentication), ("billing_error", FailureKind::Billing), ("model_not_found", FailureKind::Configuration)]
        {
            let dir = tempfile::tempdir().unwrap();
            let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
            let mut worker = check_worker(&ws, false);
            let error = json!({"type":"assistant","error":code,"message":{"content":[{"type":"text","text":"Please retry in 1s."}]}});
            let script = format!("#!/bin/sh\ncat >/dev/null\nprintf 'attempt\\n' >> attempts\nprintf '%s\\n' '{error}'\nexit 1\n");
            let mut cli = executable_cli(&ws, "claude-code", &script);
            let result = worker.turn_with_wait(&mut ws, &mut cli, "Make a notes app", true, None, |_, _| panic!("typed setup failures must not retry"));
            assert!(result.is_err(), "{kind:?}");
            assert_eq!(cli.last_failure().unwrap().kind, kind);
            assert_eq!(std::fs::read_to_string(ws.root.join("attempts")).unwrap(), "attempt\n");
            assert!(!cli.is_busy());
        }
    }

    #[cfg(unix)]
    /// OpenCode's typed rate limit is retried with the whole original request.
    #[test]
    fn opencode_retries_a_typed_rate_limit_with_its_whole_original_request() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let mut worker = check_worker(&ws, false);
        let (tx, rx) = std::sync::mpsc::channel();
        worker.tx = tx;
        let mut cli = executable_cli(
            &ws,
            "opencode",
            concat!(
                "#!/bin/sh\nif [ \"$2\" = '--help' ]; then printf '%s\\n' '--format --standalone'; exit 0; fi\n",
                // The message comes on stdin.
                "cat >> prompts; printf '\\n' >> prompts\n",
                "if [ ! -f rate-was-returned ]; then touch rate-was-returned\n",
                "printf '%s\\n' '{\"type\":\"error\",\"error\":{\"name\":\"APIError\",\"data\":{\"message\":\"Please retry in 19.052906498s.\",\"statusCode\":429,\"isRetryable\":true}}}'\nexit 1\nfi\n",
                "printf '%s\\n' '{\"type\":\"text\",\"part\":{\"text\":\"Built the app\"}}'\n"
            ),
        );
        let mut waits = Vec::new();
        assert_eq!(
            worker
                .turn_with_wait(&mut ws, &mut cli, "Make a notes app", true, None, |_, seconds| {
                    waits.push(seconds);
                    Ok(())
                })
                .unwrap(),
            "Built the app"
        );
        assert_eq!(waits, [21]);
        assert_eq!(std::fs::read_to_string(ws.root.join("prompts")).unwrap(), "Make a notes app\nMake a notes app\n");
        assert!(cli.session_id().is_none());
        assert!(cli.last_failure().is_none(), "successful retry clears the prior failure");
        assert!(!cli.is_busy());
        assert_eq!(rx.try_iter().filter(|e| matches!(e, RunEvent::Agent(AgentEvent::TurnDone { .. }))).count(), 2, "one final event per process attempt");
    }

    #[cfg(unix)]
    /// An untyped OpenCode exit keeps its error and stops at the three-retry cap.
    #[test]
    fn an_untyped_opencode_exit_keeps_its_error_and_the_three_retry_cap() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(dir.path(), "x", "Build a thing", TargetRef::None).unwrap();
        let mut worker = check_worker(&ws, false);
        let mut cli = executable_cli(
            &ws,
            "opencode",
            "#!/bin/sh\nif [ \"$2\" = '--help' ]; then printf '%s\\n' '--format --standalone'; exit 0; fi\nprintf 'attempt\\n' >> attempts\nprintf 'Unknown option: --model. Please retry in 1s.\\n' >&2\nexit 2\n",
        );
        let mut waits = Vec::new();
        let error = worker
            .turn_with_wait(&mut ws, &mut cli, "Make a notes app", true, None, |_, seconds| {
                waits.push(seconds);
                Ok(())
            })
            .unwrap_err();
        assert!(error.message.contains("Unknown option"), "{error:?}");
        assert_eq!(cli.last_failure().unwrap().kind, FailureKind::Transient);
        assert!(!cli.is_busy());
        assert_eq!(waits, [2, 2, 2], "the display wait hint affects only the delay after a retryable exit");
        assert_eq!(std::fs::read_to_string(ws.root.join("attempts")).unwrap(), "attempt\nattempt\nattempt\nattempt\n");
    }
}

#[test]
fn games_get_the_mechanics_sheet_and_decompile_only_in_deep_mode() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Game", "Copy this game", TargetRef::Local { path: d.path().join("game.exe") }).unwrap();
    let deep = brief(&ws, "game", true, false, None, false, None);
    let clean = brief(&ws, "game", false, false, None, false, None);
    for b in [&deep, &clean] {
        assert!(b.contains("MECHANICS.md"), "{b}");
    }
    // Deep mode: 1:1 with the original's own files, for private use, never decrypting anything.
    assert!(deep.contains("original game's own files") && deep.contains("Never decrypt") && deep.contains("must not be shared") && deep.contains("CREDITS.md"));
    assert!(deep.contains(PRIVATE_USE_NOTE), "the README line the credits check looks for, as written");
    // It points the builder at the app's own archive tools before any outside converter.
    assert!(deep.contains("enable_toolset with \"formats\"") && deep.contains("game_pak_extract") && deep.contains("game_unreal_texture_export"));
    assert!(!clean.contains("game_pak_extract"), "Clean room copies no files out of the game");
    // Clean room copies none of the original's files: its own art, and it says Deep mode is needed.
    assert!(clean.contains("take nothing from the original game's files") && clean.contains("Make your own simple versions"), "{clean}");
    assert!(!clean.contains("original game's own files") && !clean.contains("must not be shared") && clean.contains("needs Deep mode"));
    assert!(deep.contains("native_function_pseudocode") && deep.contains("never paste decompiled code"));
    assert!(clean.contains("don't open, read, list or decompile the game's files or code") && !clean.contains("decompile the program before"));
    assert!(clean.contains("nothing inside the program, its files or its folder was read") && !clean.contains("core_target_open"), "{clean}");
    // Deep mode requires decompiling before estimating, and says to wait for Ghidra.
    assert!(deep.contains("decompile the program before you estimate anything") && deep.contains("only after you looked for it in the code"));
    // It says which decompiler it really has.
    assert!(deep.contains("Ghidra isn't installed") && deep.contains("built-in decompiler"));
    // Generated assets replace the original's art; its sounds and the rest still come from the game.
    let mut gen_ws = ws.clone();
    gen_ws.manifest.generate_assets = true;
    let generated = brief(&gen_ws, "game", true, false, None, true, None);
    assert!(generated.contains("instead of copying the original's") && generated.contains("Realistic assets"), "{generated}");
    assert!(!generated.contains("Art, sounds, models") && generated.contains("Sounds, music, fonts, animations and maps"), "{generated}");
    // Clean room with generated assets: generated art, the rest its own; still nothing copied.
    let generated = brief(&gen_ws, "game", false, false, None, true, None);
    assert!(
        generated.contains("take nothing from the original game's files") && generated.contains("with assets_model_generate and assets_image_generate"),
        "{generated}"
    );
    assert!(generated.contains("Realistic assets") && !generated.contains("original game's own files"), "{generated}");
    let with_ghidra = brief(&ws, "game", true, true, None, false, None);
    assert!(with_ghidra.contains("reads the code with Ghidra") && with_ghidra.contains("never skip decompiling") && !with_ghidra.contains("isn't installed"));
    // An app rebuilt from a program gets the same rule in Deep mode, and only then.
    let app = brief(&ws, "app", true, true, None, false, None);
    assert!(app.contains("decompile the program before you estimate anything") && app.contains("features the person asked for"), "{app}");
    assert!(!brief(&ws, "app", false, true, None, false, None).contains("decompile the program before"));
    assert!(!clean.contains("Ghidra"), "Clean room never mentions Ghidra");
    // Without a target there is nothing to decompile or license; the sheet still helps.
    let none = Workspace::create(d.path(), "Fresh", "a snake game", TargetRef::None).unwrap();
    let b = brief(&none, "game", true, true, None, false, None);
    assert!(b.contains("MECHANICS.md") && !b.contains("decompile") && !b.contains("CREDITS.md"));
    // Every game, the snake demo too, plays its core loop for the self-test check.
    for b in [&b, &deep, &clean] {
        assert!(b.contains(kinds::self_test::SELF_TEST_CONTRACT), "{b}");
    }
    assert!(!brief(&ws, "app", false, false, None, false, None).contains("MECHANICS.md"), "apps get the sheet only in Deep mode");
}

/// What a person typed in the app when the clone came out in Vite without shadcn.
const SAID_NEXT_SHADCN: &str = "make a copy of this website, get everything right from padding, mobile responsiveness, colors. use nextjs and shadcn";

#[test]
fn a_named_stack_and_component_library_reach_the_builder() {
    let d = tempfile::tempdir().unwrap();
    let web = TargetRef::Website { url: "https://example.com".into() };
    let mut ws = Workspace::create(d.path(), "ex", SAID_NEXT_SHADCN, web.clone()).unwrap();
    ws.manifest.stack = choose(SAID_NEXT_SHADCN, &web).1.into();
    let b = brief(&ws, "website", false, false, None, false, None);
    assert!(b.contains("Use Next.js") && b.contains("output: 'export'") && b.contains("app/out") && !b.contains("Vite"), "{b}");
    assert!(b.contains("shadcn/ui components") && b.contains("npx shadcn@latest init --yes --defaults"), "{b}");
    // Without the library in the request, no shadcn; with a stack it doesn't fit, none either.
    ws.manifest.request = "Clone this website in Next.js".into();
    assert!(!brief(&ws, "website", false, false, None, false, None).contains("shadcn"));
    ws.manifest.request = "A tool in python with shadcn".into();
    ws.manifest.stack = "Python".into();
    assert!(!brief(&ws, "app", false, false, None, false, None).contains("shadcn@latest"));
}

#[test]
fn prompts_carry_the_request_rules_and_measurements() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "ex", "Clone this website", TargetRef::Website { url: "https://example.com".into() }).unwrap();
    ws.manifest.stack = "React + TypeScript (Vite)".into();
    let b = brief(&ws, "website", false, false, None, false, Some("make it blue"));
    assert!(b.contains("Clone this website") && b.contains("https://example.com") && b.contains("core_person_ask") && b.contains("npm run build"));
    assert!(b.contains("make it blue") && b.contains("Never start dev servers"));
    // A part of the request that won't fit is asked about, never dropped silently.
    assert!(b.contains("before leaving it out") && b.contains("never drop it silently"));
    let r = refine("website", &CheckResult { similarity: Some(71.4), ..CheckResult::new(CheckResult::WEB_VISUAL, 2, true, "1440 px: 70% alike") });
    assert!(r.contains("71% alike") && r.contains("checks/round-2/"));
    assert!(follow_up("bigger title", "app").contains("build it again"));
}

#[test]
fn needs_an_agent_app() {
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let d = tempfile::tempdir().unwrap();
    let hub = AiHub::with_stores(engine, d.path().join("settings.toml"), crate::KeyStore::file_only(d.path().join("keys.toml")));
    let ws = Workspace::create(d.path(), "x", "y", TargetRef::None).unwrap();
    let dir = notes::notes_dir(&ws.inbox_dir());
    let left = notes::Left { text: "make it blue".into(), shown: "make it blue".into(), pictures: Vec::new() };
    notes::hand_over(&dir, 1, "make it blue", Some(&left)).unwrap();
    let err = RebuildRun::start(&hub, ws, RunOptions::default()).err().unwrap();
    assert!(err.message.contains("coding agent"), "{}", err.message);
    assert!(dir.join("1.json").is_file(), "a run that can't start leaves an earlier run's notes waiting (#1620)");
}

/// A fork's own kind: recognised from its words, with its own stack and check.
struct SlidesKind;
struct OutlineCheck;

impl Check for OutlineCheck {
    fn id(&self) -> &'static str {
        "outline"
    }
    fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
        let ok = cx.ws.app_dir().join("slides.md").is_file();
        Ok(CheckResult::new(self.id(), cx.round, ok, if ok { "Slides found." } else { "No slides.md yet." }))
    }
}

impl TargetKind for SlidesKind {
    fn id(&self) -> &'static str {
        "slides"
    }
    fn words(&self) -> KindWords {
        KindWords { button: "Open the slides", icon: "presentation", ready: "Your slides are ready", change_hint: "Ask for a change" }
    }
    fn detect(&self, cx: &DetectCx) -> Option<u32> {
        cx.says(&[" slides", " deck"]).then_some(80)
    }
    fn default_stack(&self, _cx: &DetectCx) -> &'static str {
        "Markdown slides"
    }
    fn goal(&self) -> &'static str {
        "a slide deck"
    }
    fn checks(&self, _ws: &Workspace) -> Vec<Box<dyn Check>> {
        vec![Box::new(OutlineCheck)]
    }
}

#[test]
fn a_registered_kind_is_chosen_from_its_words_and_brings_its_checks() {
    let mut kinds = KindRegistry::built_in();
    assert_eq!(kinds.ids(), ["website", "game", "app"]);
    kinds.register(Arc::new(SlidesKind));
    let (k, stack) = kinds.choose("Make slides about our product", &TargetRef::None, true).unwrap();
    assert_eq!((k.id(), stack), ("slides", "Markdown slides"));
    assert_eq!(kinds.choose("Make a website about our product", &TargetRef::None, true).unwrap().0.id(), "website", "built-ins still win their own words");
    assert_eq!(kinds.choose("A notes tool", &TargetRef::None, true).unwrap().0.id(), "app", "app is still the fallback");
    // Registering the same id again replaces it.
    kinds.register(Arc::new(SlidesKind));
    assert_eq!(kinds.ids(), ["website", "game", "app", "slides"]);

    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Deck", "Make slides", TargetRef::None).unwrap();
    let checks = kinds.get("slides").unwrap().checks(&ws);
    assert_eq!(checks.iter().map(|c| c.id()).collect::<Vec<_>>(), ["outline"]);
    let b =
        super::brief(&SlidesKind, &BriefCx { ws: &ws, deep: false, ghidra: false, editor: None, assets: false, image_model: None, sees_pictures: true }, None);
    assert!(b.contains("What to build: a slide deck."), "{b}");
}

/// An OpenCode builder kept its own progress notes in inbox/build-progress.json, which stopped every
/// run after it: every brief names Craft Anything's records, by the names the workspace uses.
#[test]
fn every_brief_names_the_records_a_builder_must_not_write() {
    use craft_anything_engine::workspace::{MANIFEST, OWN_NAMES, progress::BUILD_PROGRESS_FILE};
    let d = tempfile::tempdir().unwrap();
    for k in ["app", "game", "website"] {
        let ws = Workspace::create(d.path(), k, "A thing", TargetRef::None).unwrap();
        let b = brief(&ws, k, false, false, None, false, None);
        let records = format!("Craft Anything's own records there ({MANIFEST}, {}, everything in inbox/)", craft_anything_engine::feed::FEED_FILE);
        assert!(b.contains(&records), "{k}: {b}");
    }
    assert!(BUILD_PROGRESS_FILE.starts_with("inbox/") && OWN_NAMES.contains(&"inbox"), "the brief's inbox/ is where the record is");
}

#[test]
fn the_brief_puts_the_request_before_the_default_stack() {
    let d = tempfile::tempdir().unwrap();
    for (k, stack) in [("app", "Rust + egui (eframe)"), ("game", "Unity"), ("website", stacks::BUILDERS_CHOICE)] {
        let mut ws = Workspace::create(d.path(), k, "A music player in Tauri", TargetRef::None).unwrap();
        ws.manifest.stack = stack.into();
        let b = brief(&ws, k, false, false, Some(Path::new("/ed/Unity")), false, None);
        let at = |s: &str| b.find(s).unwrap_or_else(|| panic!("{k}: no {s:?} in {b}"));
        assert!(at("Their request: \"A music player in Tauri\"") < at("How to build it, unless their request says otherwise: "), "{k}");
        assert!(b.contains("Their request comes first. When it says how to build"), "{k}");
        assert!(!b.contains("npm run build` again"), "{k}");
    }
}

#[test]
fn an_electron_app_starts_in_its_own_electron_and_is_no_website() {
    let d = tempfile::tempdir().unwrap();
    let app = d.path();
    std::fs::write(app.join("package.json"), r#"{"main": "main.js", "devDependencies": {"electron": "38", "vite": "7"}}"#).unwrap();
    std::fs::create_dir_all(app.join("dist")).unwrap();
    std::fs::write(app.join("dist/index.html"), "x").unwrap();
    assert!(site_dir(app).is_none(), "its bundled page is its window, not a site");
    assert!(launch(app, None).is_none(), "nothing to start before npm install");
    let dist = app.join("node_modules/electron/dist");
    let exe = if cfg!(windows) {
        dist.join("electron.exe")
    } else if cfg!(target_os = "macos") {
        dist.join("Electron.app/Contents/MacOS/Electron")
    } else {
        dist.join("electron")
    };
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, "").unwrap();
    assert_eq!(launch(app, None), Some((exe, vec![app.display().to_string()])));
    assert_eq!(stacks::used_in(app).map(|s| s.name.as_str()), Some("Electron"), "recorded as Electron, not Vite");
    std::fs::write(app.join("package.json"), "not json").unwrap();
    assert!(launch(app, None).is_none() && site_dir(app).is_some(), "a broken package.json is no Electron app");
}

#[test]
fn website_kind_compares_only_when_there_is_an_original() {
    let d = tempfile::tempdir().unwrap();
    let site = Workspace::create(d.path(), "Site", "Clone it", TargetRef::Website { url: "https://example.com".into() }).unwrap();
    let scratch = Workspace::create(d.path(), "Page", "A landing page", TargetRef::None).unwrap();
    let ids = |ws: &Workspace| kinds::WebsiteKind.checks(ws).iter().map(|c| c.id()).collect::<Vec<_>>();
    assert_eq!(ids(&site), [CheckResult::WEB_VISUAL]);
    assert_eq!(ids(&scratch), [CheckResult::BUILD]);
}

/// #494: the website check, run for real with no site to look at, fails with a note that names the
/// places a site is accepted and where the app is: `app/`, or the project folder for a project
/// that edits its own folder.
#[test]
fn the_website_check_with_no_site_says_where_it_looked() {
    let d = tempfile::tempdir().unwrap();
    for in_root in [false, true] {
        let mut ws = Workspace::create(
            &d.path().join(if in_root { "root" } else { "app" }),
            "Site",
            "Clone it",
            TargetRef::Website { url: "https://example.com".into() },
        )
        .unwrap();
        ws.manifest.app_in_root = in_root;
        let place = if in_root { "the project folder" } else { "app/" };
        let r = check_worker(&ws, false).run_checks(&ws, &kinds::WebsiteKind, 1).unwrap();
        assert_eq!(r.len(), 1);
        assert!(!r[0].passed && r[0].check == CheckResult::WEB_VISUAL, "{:?}", r[0]);
        let note = &r[0].note;
        assert!(note.contains(&format!("No built site found in {place} yet")), "{note}");
        assert!(note.contains(&format!("index.html straight in {place}")) && note.contains("dist/, build/ or out/"), "{note}");
        assert!(in_root || !note.contains("project folder"), "{note}");
    }
}

#[test]
fn a_picked_game_is_rebuilt_with_its_own_engine_when_we_can() {
    let d = tempfile::tempdir().unwrap();
    let game = d.path().join("Arena");
    std::fs::create_dir_all(game.join("Content").join("Paks")).unwrap();
    std::fs::write(game.join("Content").join("Paks").join("Arena-Windows.pak"), b"pak").unwrap();
    let target = TargetRef::Local { path: game.clone() };
    assert_eq!(choose("Recreate this game", &target), ("game", "Unreal Engine"), "an Unreal game is rebuilt in Unreal");
    assert_eq!(choose("Recreate this game in Godot", &target).1, "Godot", "an engine the person names still wins");
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(choose("Recreate this game", &TargetRef::Local { path: empty.path().to_path_buf() }).1, "Unity", "no engine found: Unity");
}

/// A scripted API model builds with the Build tools in one turn, and leaves no session to resume.
#[test]
fn an_api_backend_builds_with_the_build_tools() {
    use crate::agent::tests::{Scripted, turn};
    use crate::agent::{ApiConversation, EngineHost, LoopConfig, ModelSlot};
    use crate::model::{Block, StopReason};
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: engine.clone(),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    w.attach(&ws).unwrap();
    // The model writes a file with the Build tools, then answers.
    let write = Block::ToolUse { id: "w".into(), name: "build_file_write".into(), input: serde_json::json!({"path": "app/README.md", "content": "# Notes\n"}) };
    let script = Scripted {
        turns: std::collections::VecDeque::from([
            turn(vec![write], StopReason::ToolUse),
            turn(vec![Block::Text { text: "Built the notes app.".into() }], StopReason::EndTurn),
        ]),
        seen: Arc::default(),
    };
    let slot =
        ModelSlot { name: "Scripted".into(), model: Box::new(script), local: false, effort: None, max_turns: 10, context_tokens: 0, tools: Default::default() };
    let enabled = std::collections::BTreeSet::from([craft_anything_engine::Toolset::Core, craft_anything_engine::Toolset::Build]);
    let mut api = ApiConversation::start_with(Arc::new(EngineHost(engine)), vec![slot], LoopConfig::default(), enabled).unwrap();
    let reply = w.turn(&mut ws, &mut api, "Build it.", true, None).unwrap();
    assert_eq!(reply.trim(), "Built the notes app.");
    assert_eq!(std::fs::read_to_string(ws.app_dir().join("README.md")).unwrap(), "# Notes\n");
    assert!(ws.manifest.agent_session.is_none(), "an API loop has no session to resume");
}

#[test]
fn the_builder_is_an_agent_app_first_then_any_ai_backend() {
    let none = crate::AiConfig::default();
    assert!(builder_backend(&none, None).unwrap_err().message.contains("API key"));
}

#[test]
fn other_pages_of_the_same_site_are_picked_from_its_links() {
    use kinds::website::{ROUTES_FILE, pick_routes, routes};
    let links = serde_json::json!({"links": [
        {"href": "https://shop.example.com/pricing", "text": "Pricing"},
        {"href": "https://shop.example.com/pricing/#plans", "text": "Plans"},
        {"href": "https://SHOP.example.com/about?ref=nav", "text": "About"},
        {"href": "https://shop.example.com/", "text": "Home"},
        {"href": "https://shop.example.com/brochure.pdf", "text": "PDF"},
        {"href": "https://other.example.com/blog", "text": "Elsewhere"},
        {"href": "mailto:hi@example.com", "text": "Mail"},
        {"href": "https://shop.example.com/docs/getting-started.html", "text": "Docs"},
        {"href": "https://shop.example.com/team", "text": "Team"},
        {"href": "https://shop.example.com/jobs", "text": "Jobs"},
    ]});
    let r = pick_routes(&links, "https://shop.example.com/", 4);
    let paths: Vec<&str> = r.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, ["/pricing", "/about", "/docs/getting-started.html", "/team"], "same site, pages only, each once, at most 4");
    assert_eq!(r[2].slug, "docs-getting-started-html");
    assert_eq!(r[1].url, "https://shop.example.com/about");
    assert!(pick_routes(&links, "not a url", 4).is_empty());

    // The brief mentions the other pages only when some were captured.
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Shop", "Clone it", TargetRef::Website { url: "https://shop.example.com/".into() }).unwrap();
    let cx = |ws| BriefCx { ws, deep: false, ghidra: false, editor: None, assets: false, image_model: None, sees_pictures: true };
    // #1590: and it always says how to run the site without Craft Anything.
    assert_eq!(kinds::WebsiteKind.brief_sections(&cx(&ws)), [kinds::website::HOW_TO_RUN]);
    std::fs::create_dir_all(ws.capture_dir()).unwrap();
    std::fs::write(ws.capture_dir().join(ROUTES_FILE), serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(routes(&ws).len(), 4);
    let s = kinds::WebsiteKind.brief_sections(&cx(&ws)).join(" ");
    assert!(s.contains("/pricing (capture/routes/pricing/)"), "{s}");

    // Opened menus, pop-ups and tabs are listed when the study pictured some.
    let states = ws.capture_dir().join(kinds::website::STATES_DIR);
    std::fs::create_dir_all(&states).unwrap();
    let list = json!({"states": [
        {"kind": "menu", "label": "Products", "file": "state-1.png"},
        {"kind": "dialog", "label": "Sign up", "changed": ["address changed"]}
    ]});
    std::fs::write(states.join("states.json"), serde_json::to_vec(&list).unwrap()).unwrap();
    let s = kinds::WebsiteKind.brief_sections(&cx(&ws)).join(" ");
    assert!(s.contains("state-1.png (Products, menu)") && !s.contains("Sign up"), "only pictured states: {s}");
}

/// Only the mechanics check, so a test needn't build anything first.
struct SheetKind;

impl TargetKind for SheetKind {
    fn id(&self) -> &'static str {
        "sheet"
    }
    fn words(&self) -> KindWords {
        KindWords { button: "Play", icon: "play", ready: "Ready", change_hint: "Ask for a change" }
    }
    fn detect(&self, _cx: &DetectCx) -> Option<u32> {
        None
    }
    fn default_stack(&self, _cx: &DetectCx) -> &'static str {
        "Rust"
    }
    fn goal(&self) -> &'static str {
        "a game"
    }
    fn checks(&self, _ws: &Workspace) -> Vec<Box<dyn Check>> {
        vec![Box::new(MechanicsCheck)]
    }
}

#[test]
fn builder_instructions_allow_an_unverified_comparison_to_remain_open() {
    for deep in [false, true] {
        let steps = super::brief::steps(deep);
        assert!(!steps.contains("until it says it would pass"), "{steps}");
        assert!(steps.contains("another builder turn cannot resolve"), "{steps}");
    }
    assert!(CHECK_BEFORE_REPLYING.contains("unverified comparisons"), "{CHECK_BEFORE_REPLYING}");
}

#[test]
fn the_mechanics_sheet_is_required_without_claiming_effective_behavior() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let (tx, _rx) = std::sync::mpsc::channel();
    let w = Worker {
        wake: Default::default(),
        engine,
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let check = |round| w.run_checks(&ws, &SheetKind, round).unwrap().remove(0);

    let none = check(1);
    assert!(!none.passed && none.note.contains("no app/mechanics.json") && none.note.contains("\"version\": 1"), "{}", none.note);
    assert!(MechanicsCheck.can_refine(&none) && kinds::DeepOnly(MechanicsCheck).can_refine(&none));

    std::fs::create_dir_all(ws.app_dir().join("src")).unwrap();
    std::fs::write(ws.mechanics_path(), "{ not json").unwrap();
    assert!(!check(2).passed, "an unreadable sheet fails");

    std::fs::write(ws.app_dir().join("src/main.rs"), "const WALK: f32 = 4.5;\nconst GRAVITY: f32 = -9.8;\n").unwrap();
    std::fs::write(
        ws.mechanics_path(),
        r#"{"version": 1, "values": [
            {"name": "walk speed", "value": 4.5, "unit": "m/s", "source": "function 0x140012a0"},
            {"name": "gravity", "value": -9.8, "estimated": true},
            {"name": "jump height", "value": 1.2, "unit": "m", "estimated": true},
            {"name": "crouch", "value": "hold"}
        ]}"#,
    )
    .unwrap();
    let ok = check(3);
    assert!(ok.passed, "an estimate may stay out of the code: {}", ok.note);
    assert!(ok.note.contains("4 values (1 with a source, 2 estimated)"), "{}", ok.note);
    assert!(ok.note.contains("neither a source nor \"estimated\": crouch."), "{}", ok.note);
    assert!(ok.claims.is_empty() && !ok.unknowns.is_empty() && !ok.good(0.0), "{ok:?}");
    assert!(!MechanicsCheck.can_refine(&ok) && !kinds::DeepOnly(MechanicsCheck).can_refine(&ok));

    // Provenance remains in the sheet; a differently implemented or missing value isn't proof
    // of a broken build, and a matching literal isn't proof of effective behavior.
    std::fs::write(ws.app_dir().join("src/main.rs"), "fn speed() -> f64 { 9.0 / 2.0 }").unwrap();
    let computed = check(4);
    assert!(computed.passed && !computed.good(0.0), "{computed:?}");
    assert!(computed.claims.is_empty() && computed.note.contains("unverified"));
}

#[test]
fn mechanics_literals_never_verify_effective_behavior() {
    for code in [
        "// walk speed 4.5\nfn speed() -> f64 { 1.0 }",
        "fn unused() -> f64 { 4.5 }\nfn speed() -> f64 { 1.0 }",
        "const UNRELATED: f64 = 4.5;\nfn speed() -> f64 { 1.0 }",
        "fn speed() -> f64 { 4.5 * 2.0 }",
        "fn speed() -> f64 { 9.0 / 2.0 }",
        "",
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
        std::fs::create_dir_all(ws.app_dir()).unwrap();
        std::fs::write(ws.app_dir().join("main.rs"), code).unwrap();
        if code.is_empty() {
            std::fs::write(ws.app_dir().join("unrelated.json"), r#"{"release":4.5}"#).unwrap();
        }
        std::fs::write(ws.mechanics_path(), r#"{"version":1,"values":[{"name":"walk speed","value":4.5,"unit":"m/s","source":"reference measurement"}]}"#)
            .unwrap();
        let c = check_worker(&ws, false).run_checks(&ws, &SheetKind, 1).unwrap().remove(0);
        assert!(c.passed, "a valid sheet is readable even with computed values: {code}: {c:?}");
        assert!(!c.good(0.0), "source text does not verify behavior: {code}: {c:?}");
        assert!(c.claims.is_empty(), "no behavior claim can be inferred: {c:?}");
        assert!(!c.unknowns.is_empty(), "no oracle was run: {c:?}");
        ws.manifest.checks.push(c.clone());
        ws.save().unwrap();
        let saved = Workspace::open(&ws.root).unwrap();
        assert_eq!(saved.manifest.checks.last().unwrap().unknowns, c.unknowns);
        let summary = self_check::summary(&[CheckResult::new("build", 1, true, "Build succeeded."), c], 90.0);
        assert!(summary.contains("unverified"), "a usable build's final summary retains the gap: {summary}");
    }
}

#[cfg(unix)]
#[test]
fn a_measured_self_test_does_not_stand_in_for_a_mechanics_comparison() {
    use std::os::unix::fs::PermissionsExt;
    for (calculation, expected) in [("$((9 * 10 / 2))", true), ("90 # unrelated 4.5", false)] {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
        std::fs::create_dir_all(ws.app_dir().join("target/debug")).unwrap();
        std::fs::write(ws.app_dir().join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(ws.mechanics_path(), r#"{"values":[{"name":"walk speed","value":4.5,"source":"controlled reference"}]}"#).unwrap();
        let program = ws.app_dir().join("target/debug/fixture");
        // This owned fixture measures integer tenths against a controlled 4.5 m/s reference.
        // Its report still cannot establish a general binding between sheet entries and state.
        let script = format!(
            r#"#!/bin/sh
tenths={calculation}
passed=false
if [ "$tenths" -eq 45 ]; then passed=true; fi
printf '{{"version":1,"steps":[{{"step":"move","passed":%s}},{{"step":"act","passed":true}},{{"step":"hud","passed":true}}]}}' "$passed" > "$2"
"#
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let checks = check_worker(&ws, false).run_checks(&ws, &LaterChecks(&GameKind, 2), 1).unwrap();
        assert_eq!(checks[0].check, "game-self-test");
        assert_eq!(checks[0].good(0.0), expected, "{checks:?}");
        if expected {
            assert_eq!(checks.len(), 2);
            assert_eq!(checks[1].check, "mechanics");
            assert!(checks[1].passed && !checks[1].good(0.0), "even a measured self-test doesn't compare every sheet value: {checks:?}");
            assert!(checks[1].claims.is_empty() && !checks[1].unknowns.is_empty());
        } else {
            assert_eq!(checks.len(), 1, "a failed measured core-loop step stops later checks");
        }
    }
}

#[test]
fn clean_room_studies_nothing_inside_and_deep_mode_looks() {
    let d = tempfile::tempdir().unwrap();
    let game = d.path().join("Some Game");
    std::fs::create_dir_all(game.join("Game_Data")).unwrap();
    std::fs::write(game.join("Game.exe"), b"MZ").unwrap();
    std::fs::write(game.join("Game_Data/globalgamemanagers"), b"\0\0\0\0\x16\0\x002022.3.62f2\0").unwrap();
    for (deep, name) in [(false, "clean"), (true, "deep")] {
        let mut ws = Workspace::create(&d.path().join("projects"), name, "Recreate this game", TargetRef::Local { path: game.clone() }).unwrap();
        let mut w = check_worker(&ws, deep);
        // What an earlier Deep study left: a Clean room study must remove it before writing its own.
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::write(ws.capture_dir().join(super::study::HOW_IT_WORKS), "deep notes").unwrap();
        std::fs::write(ws.capture_dir().join("page.html"), "<p>deep</p>").unwrap();
        let runs = ws.capture_dir().join(kinds::runs::RUNS_DIR);
        std::fs::create_dir_all(&runs).unwrap();
        std::fs::write(runs.join("no-arguments.json"), "{}").unwrap();
        w.study(&mut ws, kind("game").as_ref(), deep).unwrap();
        let notes = std::fs::read_to_string(ws.capture_dir().join("README.md")).unwrap();
        let files: Vec<_> = std::fs::read_dir(ws.capture_dir()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        if deep {
            assert!(notes.contains("# What we found") && notes.contains("Unity"), "Deep mode looks inside: {notes}");
        } else {
            assert!(notes.starts_with("# Clean room") && !notes.contains("Unity") && !notes.contains("Game_Data"), "{notes}");
            assert_eq!(files, ["README.md"], "nothing else was written from the original");
        }
    }
}

#[test]
fn an_electron_programs_runtime_is_not_analysed() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("resources")).unwrap();
    let program = d.path().join("App.exe");
    std::fs::write(&program, b"MZ").unwrap();
    assert!(!super::study::is_electron_runtime(&program), "a program with no asar beside it is analysed");
    std::fs::write(d.path().join("resources/app.asar"), b"asar").unwrap();
    assert!(super::study::is_electron_runtime(&program));
}

#[test]
fn deep_mode_maps_the_javascript_beside_an_exe_target() {
    let d = tempfile::tempdir().unwrap();
    let app = d.path().join("App");
    std::fs::create_dir_all(app.join("resources")).unwrap();
    std::fs::write(app.join("App.exe"), b"MZ").unwrap();
    std::fs::write(app.join("resources/app.asar"), craft_anything_testkit::electron::notes_app_asar()).unwrap();
    for (name, target) in [("folder", app.clone()), ("exe", app.join("App.exe"))] {
        let mut ws = Workspace::create(&d.path().join("projects"), name, "Recreate this app", TargetRef::Local { path: target }).unwrap();
        let mut w = check_worker(&ws, true);
        w.study(&mut ws, kind("game").as_ref(), true).unwrap();
        let notes = std::fs::read_to_string(ws.capture_dir().join("README.md")).unwrap();
        assert!(notes.contains("## JavaScript application"), "{name}: {notes}");
    }
}

#[cfg(unix)]
mod picking_up_a_build {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A kind whose check can't run while `app/check-fails` still counts down, and a stand-in coding
    /// agent that notes each prompt it gets in `prompts.txt` beside the project.
    struct FlakyKind;
    struct FlakyCheck;

    impl Check for FlakyCheck {
        fn id(&self) -> &'static str {
            "flaky"
        }
        fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
            let rejection = std::fs::read_to_string(cx.ws.root.join("reject-build")).unwrap_or_default();
            if !rejection.is_empty() && (rejection != "once" || cx.ws.manifest.checks.is_empty()) {
                return Ok(CheckResult::new(self.id(), cx.round, false, "The app is still missing."));
            }
            let file = cx.ws.app_dir().join("check-fails");
            let left: u32 = std::fs::read_to_string(&file).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            if left > 0 {
                std::fs::write(&file, (left - 1).to_string()).unwrap();
                return Err(ToolError::new(ErrorCode::EngineMissing, "the WebSocket is closed"));
            }
            Ok(CheckResult::new(self.id(), cx.round, true, "Fine."))
        }
    }

    impl TargetKind for FlakyKind {
        fn id(&self) -> &'static str {
            "flaky"
        }
        fn words(&self) -> KindWords {
            KindWords { button: "Open", icon: "box", ready: "Ready", change_hint: "Ask" }
        }
        fn detect(&self, _cx: &DetectCx) -> Option<u32> {
            Some(1000)
        }
        fn default_stack(&self, _cx: &DetectCx) -> &'static str {
            "Anything"
        }
        fn goal(&self) -> &'static str {
            "a thing"
        }
        fn checks(&self, ws: &Workspace) -> Vec<Box<dyn Check>> {
            if ws.mechanics_path().is_file() { vec![Box::new(MechanicsCheck)] } else { vec![Box::new(FlakyCheck)] }
        }
    }

    struct Bench {
        _dir: tempfile::TempDir,
        hub: AiHub,
        projects: PathBuf,
        prompts: PathBuf,
    }

    fn bench() -> Bench {
        let dir = tempfile::tempdir().unwrap();
        let prompts = dir.path().join("prompts.txt");
        let agent = dir.path().join("agent.sh");
        std::fs::write(&agent, format!("#!/bin/sh\nfor last; do :; done\nprintf '%s\\n===\\n' \"$last\" >> '{}'\necho Done.\n", prompts.display())).unwrap();
        std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
        let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
        let hub = AiHub::with_stores(engine, dir.path().join("settings.toml"), crate::KeyStore::file_only(dir.path().join("keys.toml")));
        let mut cfg = hub.config();
        cfg.upsert(crate::BackendEntry {
            config: crate::BackendConfig {
                name: "Stand-in".into(),
                kind: BackendKind::AgentCli,
                model: "stand-in".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            options: crate::BackendOptions { command: Some(agent.display().to_string()), ..Default::default() },
        });
        hub.engine.session().settings.ai = cfg.to_table().unwrap();
        let projects = dir.path().join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        Bench { _dir: dir, hub, projects, prompts }
    }

    /// #923: starting a run is what loads the keys, for builders that use this session's tools.
    #[test]
    fn a_run_loads_the_saved_asset_keys_into_its_session() {
        let b = bench();
        b.hub.set_asset_key("fal", "k-1").unwrap();
        let ws = Workspace::create(&b.projects, "x", "Build a notes app", TargetRef::None).unwrap();
        let _ = try_it(&b, &ws);
        let keys = b.hub.engine.session().ext.get::<craft_anything_engine::tools::asset_tools::AssetKeys>().unwrap_or_default();
        assert_eq!(keys.get("fal"), Some("k-1"));
    }

    fn try_it(b: &Bench, ws: &Workspace) -> (bool, Vec<feed::FeedItem>) {
        try_with(b, ws, RunOptions::default())
    }

    fn try_with(b: &Bench, ws: &Workspace, mut opts: RunOptions) -> (bool, Vec<feed::FeedItem>) {
        opts.kinds.register(Arc::new(FlakyKind));
        opts.backend = Some("Stand-in".into());
        let run = RebuildRun::start(&b.hub, Workspace::open(&ws.root).unwrap(), opts).unwrap();
        let began = Instant::now();
        let mut ok = None;
        while ok.is_none() {
            assert!(began.elapsed() < Duration::from_secs(30), "the run never finished");
            match run.rx.recv_timeout(Duration::from_secs(30)) {
                Ok(RunEvent::Finished { ok: o, .. }) => ok = Some(o),
                Ok(_) => {}
                Err(e) => panic!("the run went away: {e}"),
            }
        }
        (ok.unwrap(), feed::load(&Workspace::open(&ws.root).unwrap()))
    }

    fn prompts_given(b: &Bench) -> usize {
        std::fs::read_to_string(&b.prompts).unwrap_or_default().matches("===").count()
    }

    fn studied(feed: &[feed::FeedItem]) -> bool {
        feed.iter().any(|i| matches!(i, feed::FeedItem::Step { step: Step::Study, .. }))
    }

    /// A project whose builder has worked: its session and its study are there.
    fn worked_on(b: &Bench) -> Workspace {
        let mut ws = Workspace::create(&b.projects, "x", "A thing", TargetRef::None).unwrap();
        ws.manifest.agent_session = Some("session-1".into());
        ws.manifest.stack = "Anything".into();
        // The fixture's capture was studied in the default (Deep) session.
        ws.manifest.capture_deep = Some(true);
        ws.save().unwrap();
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::create_dir_all(ws.app_dir()).unwrap();
        ws
    }

    /// #91: with the helper missing, the build stops before studying or asking the agent, and
    /// says why.
    #[test]
    fn a_missing_helper_stops_the_build_before_any_work() {
        struct NoHelper;
        impl crate::cli_agents::AgentAdapter for NoHelper {
            fn ids(&self) -> &'static [&'static str] {
                &["stand-in"]
            }
            fn args(&self, cx: &crate::cli_agents::ArgsCx) -> Vec<String> {
                vec![cx.prompt.to_string()]
            }
            fn streams_json(&self) -> bool {
                false
            }
            fn check_tool_server(&self, stop: &CancelToken) -> Result<(), ToolError> {
                crate::cli_agents::cli_can_start(None, stop)
            }
        }
        let b = bench();
        let ws = Workspace::create(&b.projects, "x", "A thing", TargetRef::None).unwrap();
        let mut adapters = crate::cli_agents::AgentAdapters::built_in();
        adapters.register(Arc::new(NoHelper));
        let run =
            RebuildRun::start(&b.hub, Workspace::open(&ws.root).unwrap(), RunOptions { adapters, backend: Some("Stand-in".into()), ..RunOptions::default() })
                .unwrap();
        let began = Instant::now();
        let finished = loop {
            assert!(began.elapsed() < Duration::from_secs(30), "the run never finished");
            match run.rx.recv_timeout(Duration::from_secs(30)) {
                Ok(RunEvent::Finished { ok, message, .. }) => break (ok, message),
                Ok(_) => {}
                Err(e) => panic!("the run went away: {e}"),
            }
        };
        assert!(!finished.0, "{finished:?}");
        assert!(finished.1.contains("craft-anything-cli"), "{finished:?}");
        assert_eq!(prompts_given(&b), 0, "the agent was asked anyway");
        assert!(!studied(&feed::load(&Workspace::open(&ws.root).unwrap())), "it studied first");
    }

    #[test]
    fn a_usable_build_keeps_unverified_mechanics_in_its_ready_summary() {
        let b = bench();
        let mut ws = worked_on(&b);
        ws.manifest.awaiting_check = true;
        std::fs::write(ws.mechanics_path(), r#"{"values":[{"name":"walk speed","value":4.5,"source":"reference measurement"}]}"#).unwrap();
        std::fs::write(ws.app_dir().join("main.rs"), "// 4.5\n").unwrap();
        ws.save().unwrap();
        let (ok, _) = try_it(&b, &ws);
        assert!(ok, "unverified mechanics do not make a usable build fail");
        assert_eq!(prompts_given(&b), 0, "a builder turn cannot supply the check's missing measurement oracle");
        let saved = Workspace::open(&ws.root).unwrap();
        assert_eq!(saved.manifest.status, Status::Ready);
        assert!(saved.manifest.note.contains("unverified"), "{}", saved.manifest.note);
        assert!(!saved.manifest.checks.last().unwrap().good(0.0));
    }

    #[test]
    fn try_again_after_a_check_that_could_not_run_only_checks() {
        let b = bench();
        let mut ws = worked_on(&b);
        ws.manifest.awaiting_check = true;
        ws.save().unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(ok, "{feed:?}");
        assert_eq!(prompts_given(&b), 0, "the builder had finished: it isn't asked again");
        assert!(!studied(&feed), "the original isn't studied again");
        let ws = Workspace::open(&ws.root).unwrap();
        assert!(!ws.manifest.awaiting_check);
        assert_eq!(ws.manifest.checks.len(), 1);
    }

    #[test]
    fn a_message_after_just_ask_tells_the_builder_what_was_asked_once() {
        use craft_anything_engine::questions::{self, AskedQuestion};
        let b = bench();
        let ws = worked_on(&b);
        let ask = |q: &str| {
            questions::add(&ws, AskedQuestion { deep: false, ..AskedQuestion::new(q.into(), "It's in src.".into(), true, "Stand-in".into(), None, 0) }).unwrap()
        };
        let say = |m: &str| {
            try_with(&b, &ws, RunOptions { message: Some(m.into()), ..RunOptions::default() });
            std::fs::read_to_string(&b.prompts).unwrap_or_default()
        };
        ask("where is the score kept");
        let told = say("make it bigger");
        assert!(told.contains("Q: where is the score kept\nA: It's in src.") && told.contains("make it bigger"), "{told}");
        let again = say("and blue");
        let later = again.strip_prefix(told.as_str()).unwrap_or_default();
        assert!(later.contains("and blue") && !later.contains("where is the score kept"), "told once: {later}");
        // Try again has no new message for it to go with, so it isn't used up.
        ask("how does saving work");
        let _ = try_it(&b, &ws);
        let before = std::fs::read_to_string(&b.prompts).unwrap_or_default();
        assert!(!before.contains("how does saving work"), "{before}");
        let told = say("now add saving");
        let later = told.strip_prefix(before.as_str()).unwrap_or_default();
        assert!(later.contains("how does saving work") && later.contains("now add saving"), "{later}");
    }

    /// A turn that failed may never have reached the builder, so its questions are still to tell.
    #[test]
    fn a_failed_turn_leaves_just_ask_questions_to_tell_with_the_next_message() {
        use craft_anything_engine::questions::{self, AskedQuestion};
        let b = bench();
        let ws = worked_on(&b);
        questions::add(
            &ws,
            AskedQuestion { deep: false, ..AskedQuestion::new("where is the score kept".into(), "In src.".into(), true, "Stand-in".into(), None, 0) },
        )
        .unwrap();
        // As in the test above: a typed failure, which isn't retried after a wait.
        let mut config = b.hub.config();
        let mut entry = config.get("Stand-in").unwrap().clone();
        entry.config.model = "opencode".into();
        config.upsert(entry);
        b.hub.engine.session().settings.ai = config.to_table().unwrap();
        let agent = b._dir.path().join("agent.sh");
        let works = std::fs::read_to_string(&agent).unwrap().replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\nif [ \"$2\" = '--help' ]; then printf '%s\\n' '--format --standalone'; exit 0; fi\n",
            1,
        );
        let error = json!({"type":"error","error":{"name":"APIError","data":{"message":"fixture sign in needed","statusCode":401,"isRetryable":false}}});
        std::fs::write(&agent, works.replace("echo Done.", &format!("printf '%s\\n' '{error}'\nexit 1"))).unwrap();
        let say = |m: &str| try_with(&b, &ws, RunOptions { message: Some(m.into()), ..RunOptions::default() }).0;
        assert!(!say("make it bigger"));
        assert_eq!(questions::untold(&ws, false).len(), 1, "the turn failed");
        std::fs::write(&agent, works).unwrap();
        say("make it bigger");
        assert!(questions::untold(&ws, false).is_empty(), "the turn finished");
    }

    #[test]
    fn try_again_after_the_builder_stopped_carries_on_instead_of_starting_over() {
        let b = bench();
        let ws = worked_on(&b);
        let (ok, feed) = try_it(&b, &ws);
        assert!(ok, "{feed:?}");
        assert!(!studied(&feed), "the original isn't studied again");
        let told = std::fs::read_to_string(&b.prompts).unwrap();
        assert!(told.contains("Pick up where you left off"), "{told}");
        assert!(!told.contains("Their request"), "not the first-build brief again: {told}");
    }

    #[test]
    fn unchanged_builder_turns_stop_after_two_and_stay_stopped_when_reopened() {
        let b = bench();
        let ws = worked_on(&b);
        std::fs::write(ws.root.join("reject-build"), "not ready").unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(!ok, "{feed:?}");
        assert_eq!(prompts_given(&b), 2, "a third unchanged turn would spend more without progress");
        assert!(
            feed.iter().any(|i| matches!(i, feed::FeedItem::Error { message, .. } if message.contains("two builder turns"))),
            "the stop explains what happened: {feed:?}"
        );
        let reopened = Workspace::open(&ws.root).unwrap();
        let (ok, _) = try_it(&b, &reopened);
        assert!(!ok);
        assert_eq!(prompts_given(&b), 2, "Try again must keep the saved progress limit");
    }

    #[test]
    fn single_turn_runs_share_the_same_unchanged_turn_limit() {
        let b = bench();
        let ws = worked_on(&b);
        std::fs::write(ws.root.join("reject-build"), "not ready").unwrap();
        for expected in [1, 2, 2] {
            let (ok, _) = try_with(&b, &ws, RunOptions { max_rounds: 0, ..RunOptions::default() });
            assert!(!ok);
            assert_eq!(prompts_given(&b), expected);
        }
    }

    #[test]
    fn a_real_edit_or_nonempty_new_request_starts_another_attempt() {
        let b = bench();
        let ws = worked_on(&b);
        std::fs::write(ws.root.join("reject-build"), "not ready").unwrap();
        assert!(!try_it(&b, &ws).0);
        assert_eq!(prompts_given(&b), 2);
        std::fs::write(ws.app_dir().join("index.html"), "a new starting point").unwrap();
        assert!(!try_it(&b, &ws).0);
        assert_eq!(prompts_given(&b), 4, "a real edit reset the previous limit");
        let opts = RunOptions { message: Some("Use the new index.html and add a working button.".into()), ..RunOptions::default() };
        assert!(!try_with(&b, &ws, opts).0);
        assert_eq!(prompts_given(&b), 6, "a new request starts another attempt");
        let empty = RunOptions { message: Some("   ".into()), ..RunOptions::default() };
        assert!(!try_with(&b, &ws, empty).0);
        assert_eq!(prompts_given(&b), 6, "an empty message cannot erase the saved count");
    }

    #[test]
    fn a_successful_check_can_finish_the_second_unchanged_turn() {
        let b = bench();
        let ws = worked_on(&b);
        std::fs::write(ws.root.join("reject-build"), "once").unwrap();
        assert!(try_it(&b, &ws).0);
        assert_eq!(prompts_given(&b), 2);
        assert_eq!(BuildProgress::load(&ws.root).unwrap().unchanged_turns, 2);
    }

    #[test]
    fn a_saved_check_only_run_is_not_blocked_by_the_progress_limit() {
        let b = bench();
        let mut ws = worked_on(&b);
        ws.manifest.awaiting_check = true;
        ws.save().unwrap();
        let mut progress = BuildProgress::load(&ws.root).unwrap();
        progress.before_turn(ws.app_fingerprint(&CancelToken::default()).unwrap());
        progress.unchanged_turns = 2;
        progress.save(&ws.root).unwrap();
        assert!(try_it(&b, &ws).0);
        assert_eq!(prompts_given(&b), 0);
    }

    #[test]
    fn a_failed_check_only_resume_detects_manual_edits_before_refusing_more_build_turns() {
        for edited in [false, true] {
            let b = bench();
            let mut ws = worked_on(&b);
            std::fs::write(ws.root.join("reject-build"), "not ready").unwrap();
            ws.manifest.awaiting_check = true;
            ws.save().unwrap();
            let mut progress = BuildProgress::load(&ws.root).unwrap();
            progress.before_turn(ws.app_fingerprint(&CancelToken::default()).unwrap());
            progress.unchanged_turns = 2;
            progress.save(&ws.root).unwrap();
            if edited {
                std::fs::write(ws.app_dir().join("index.html"), "a manual correction").unwrap();
            }
            assert!(!try_it(&b, &ws).0);
            assert_eq!(prompts_given(&b), if edited { 2 } else { 0 }, "edited={edited}");
            assert_eq!(BuildProgress::load(&ws.root).unwrap().unchanged_turns, 2);
        }
    }

    #[test]
    fn scan_io_errors_skip_only_the_guard_and_cancellation_is_never_skipped() {
        for fail_scan_first in [false, true] {
            let b = bench();
            let ws = worked_on(&b);
            let worker = check_worker(&ws, false);
            let mut progress = BuildProgress::default();
            progress.before_turn(ws.app_fingerprint(&CancelToken::default()).unwrap());
            progress.unchanged_turns = 2;
            progress.save(&ws.root).unwrap();
            let mut available = true;
            let mut fingerprints = AppFingerprintCache::default();
            if fail_scan_first {
                // A deterministic read_dir I/O failure, including when tests run as root.
                std::fs::remove_dir(ws.app_dir()).unwrap();
                std::fs::write(ws.app_dir(), "the app directory is temporarily unavailable").unwrap();
                assert!(worker.progress_fingerprint(&ws, &mut progress, &mut available, &mut fingerprints).unwrap().is_none());
                assert!(!available);
                assert_eq!(progress, BuildProgress::default());
                assert_eq!(BuildProgress::load(&ws.root).unwrap(), progress);
                std::fs::remove_file(ws.app_dir()).unwrap();
                std::fs::create_dir(ws.app_dir()).unwrap();
                assert!(
                    worker.progress_fingerprint(&ws, &mut progress, &mut available, &mut fingerprints).unwrap().is_none(),
                    "a recovered directory does not resume comparisons within the same run"
                );
            }
            let before = progress.clone();
            worker.stop.cancel();
            assert_eq!(worker.progress_fingerprint(&ws, &mut progress, &mut available, &mut fingerprints).unwrap_err().code, ErrorCode::Cancelled);
            assert_eq!(progress, before, "cancellation does not erase a known comparison");
            assert_eq!(BuildProgress::load(&ws.root).unwrap(), before);
        }
    }

    #[test]
    fn a_skipped_scan_does_not_hide_the_builders_own_failure() {
        let b = bench();
        let ws = worked_on(&b);
        let nested = (0..65).fold(ws.app_dir().join("nested"), |p, _| p.join("d"));
        std::fs::create_dir_all(nested).unwrap();
        // Authentication is a typed provider failure. A plain exit 1 is transient, regardless
        // of the English words it prints, and is covered by the retry-policy tests.
        let mut config = b.hub.config();
        let mut entry = config.get("Stand-in").unwrap().clone();
        entry.config.model = "opencode".into();
        config.upsert(entry);
        b.hub.engine.session().settings.ai = config.to_table().unwrap();
        let agent = b._dir.path().join("agent.sh");
        let script = std::fs::read_to_string(&agent).unwrap();
        let script = script.replacen("#!/bin/sh\n", "#!/bin/sh\nif [ \"$2\" = '--help' ]; then printf '%s\\n' '--format --standalone'; exit 0; fi\n", 1);
        let error = json!({"type":"error","error":{"name":"APIError","data":{"message":"fixture sign in needed","statusCode":401,"isRetryable":false}}});
        std::fs::write(&agent, script.replace("echo Done.", &format!("printf '%s\\n' '{error}'\nexit 1"))).unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(!ok);
        assert_eq!(prompts_given(&b), 1, "the unavailable scan does not block or repeat the builder: {feed:?}");
        assert!(feed.iter().any(|i| matches!(i, feed::FeedItem::Error { message, .. } if message.contains("fixture sign in needed"))), "{feed:?}");
        let saved = Workspace::open(&ws.root).unwrap();
        assert!(!saved.manifest.awaiting_check, "a failed turn has not finished its build");
        assert!(saved.manifest.checks.is_empty(), "the failed builder is not described as a successful check");
    }

    #[test]
    fn scan_limits_do_not_block_builds_or_keep_a_stale_stall_count() {
        for only_check in [false, true] {
            let b = bench();
            let mut ws = worked_on(&b);
            let mut progress = BuildProgress::default();
            progress.before_turn(ws.app_fingerprint(&CancelToken::default()).unwrap());
            progress.unchanged_turns = 2;
            progress.save(&ws.root).unwrap();
            ws.manifest.awaiting_check = only_check;
            ws.save().unwrap();
            let nested = (0..65).fold(ws.app_dir().join("nested"), |p, _| p.join("d"));
            std::fs::create_dir_all(&nested).unwrap();
            assert_eq!(ws.app_fingerprint(&CancelToken::default()).unwrap_err().code, ErrorCode::OverLimit);
            std::fs::write(ws.root.join("reject-build"), "once").unwrap();
            let (ok, feed) = try_it(&b, &ws);
            assert!(ok, "a scan limit must not fail the build (only_check={only_check}): {feed:?}");
            assert_eq!(prompts_given(&b), if only_check { 1 } else { 2 });
            assert_eq!(BuildProgress::load(&ws.root).unwrap(), BuildProgress::default(), "an unknown comparison breaks the saved streak");
            let notices = feed
                .iter()
                .filter_map(|i| match i {
                    feed::FeedItem::Work { notes, .. } => Some(notes.iter().filter(|n| n.contains("without the unchanged-turn limit")).count()),
                    _ => None,
                })
                .sum::<usize>();
            assert_eq!(notices, 1, "one notice explains that this run skipped the unavailable guard");
            assert_eq!(Workspace::open(&ws.root).unwrap().manifest.status, Status::Ready);
            std::fs::remove_dir_all(ws.app_dir().join("nested")).unwrap();
            assert!(try_it(&b, &ws).0);
            assert_eq!(BuildProgress::load(&ws.root).unwrap().unchanged_turns, 1, "the next readable run starts a new comparison");
        }
    }

    #[test]
    fn a_post_turn_scan_limit_keeps_the_finished_turn_for_check_only_recovery() {
        let b = bench();
        let ws = worked_on(&b);
        std::fs::write(ws.app_dir().join("check-fails"), "2").unwrap();
        let nested = (0..65).fold(ws.app_dir().join("nested"), |p, _| p.join("d"));
        let agent = b._dir.path().join("agent.sh");
        let script = std::fs::read_to_string(&agent).unwrap();
        std::fs::write(&agent, format!("{script}\nmkdir -p '{}'\n", nested.display())).unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(!ok, "the original check failure still stops the run");
        let saved = Workspace::open(&ws.root).unwrap();
        assert!(saved.manifest.awaiting_check, "the successful builder turn is saved before an unavailable scan");
        assert_eq!(saved.manifest.last_reply, "Done.");
        assert_eq!(prompts_given(&b), 1);
        assert_eq!(BuildProgress::load(&ws.root).unwrap(), BuildProgress::default());
        assert!(
            feed.iter().any(|i| matches!(i, feed::FeedItem::Error { message, .. } if message.contains("The build is finished, but it couldn't be checked"))),
            "{feed:?}"
        );
        assert!(try_it(&b, &saved).0, "Try again can finish the checks while the scan is still unavailable");
        assert_eq!(prompts_given(&b), 1, "the completed builder turn is not repeated");
        assert!(!Workspace::open(&ws.root).unwrap().manifest.awaiting_check);
    }

    #[test]
    fn an_api_builder_has_the_same_two_unchanged_turn_limit() {
        use crate::agent::tests::{Scripted, turn};
        use crate::agent::{ApiConversation, EngineHost, LoopConfig, ModelSlot};
        use crate::model::{Block, StopReason};
        let b = bench();
        let mut ws = worked_on(&b);
        std::fs::write(ws.root.join("reject-build"), "not ready").unwrap();
        let seen = Arc::default();
        let script = Scripted {
            turns: (0..5).map(|_| turn(vec![Block::Text { text: "No files changed.".into() }], StopReason::EndTurn)).collect(),
            seen: Arc::clone(&seen),
        };
        let mut worker = check_worker(&ws, false);
        let slot = ModelSlot {
            name: "Scripted".into(),
            model: Box::new(script),
            local: true,
            effort: None,
            max_turns: 10,
            context_tokens: 0,
            tools: Default::default(),
        };
        let api = ApiConversation::start(Arc::new(EngineHost(worker.engine.clone())), vec![slot], LoopConfig::default()).unwrap();
        let entry = crate::BackendEntry {
            config: crate::BackendConfig {
                name: "Scripted".into(),
                kind: BackendKind::OpenaiCompatible,
                model: "scripted".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            options: Default::default(),
        };
        let mut opts = RunOptions::default();
        opts.kinds.register(Arc::new(FlakyKind));
        let error = worker.run(&mut ws, entry, Some(api), None, super::super::KeptHistory { in_mode: true, elsewhere: false }, &opts).unwrap_err();
        assert!(error.message.contains("two builder turns"), "{error:?}");
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn new_notes_during_a_turn_or_check_get_their_own_attempt_but_empty_notes_do_not() {
        use crate::agent::tests::{Scripted, turn};
        use crate::agent::{ApiConversation, EngineHost, LoopConfig, ModelSlot};
        use crate::model::{Block, ChatModel, Delta, Stop, StopReason, TurnRequest, TurnResult};
        type Notes = Arc<Mutex<Vec<Note>>>;
        fn note(text: &str) -> Note {
            Note { id: 42, text: text.into(), shown: text.into(), pictures: Vec::new(), questions: Vec::new() }
        }
        struct NoteModel {
            script: Scripted,
            notes: Notes,
            add: bool,
            claim: bool,
            dir: std::path::PathBuf,
            text: &'static str,
        }
        impl ChatModel for NoteModel {
            fn turn(&mut self, req: &TurnRequest<'_>, delta: &mut dyn FnMut(Delta), stop: &Stop) -> Result<TurnResult, ToolError> {
                if self.add && self.script.seen.lock().unwrap().len() == 1 {
                    self.notes.lock().unwrap().push(note(self.text));
                    if self.claim {
                        notes::hand_over(&self.dir, 42, self.text, None).unwrap();
                        assert_eq!(notes::claim(&self.dir), [self.text]);
                    }
                }
                self.script.turn(req, delta, stop)
            }
            fn model(&self) -> String {
                self.script.model()
            }
        }
        struct NoteCheck {
            notes: Notes,
            add: bool,
            text: &'static str,
        }
        impl Check for NoteCheck {
            fn id(&self) -> &'static str {
                "note-check"
            }
            fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
                if self.add && cx.round == 2 {
                    self.notes.lock().unwrap().push(note(self.text));
                }
                Ok(CheckResult::new(self.id(), cx.round, false, "The app is still missing."))
            }
        }
        struct NoteKind {
            notes: Notes,
            add: bool,
            text: &'static str,
        }
        impl TargetKind for NoteKind {
            fn id(&self) -> &'static str {
                "note-kind"
            }
            fn words(&self) -> KindWords {
                FlakyKind.words()
            }
            fn detect(&self, _: &DetectCx) -> Option<u32> {
                Some(1001)
            }
            fn default_stack(&self, _: &DetectCx) -> &'static str {
                "Anything"
            }
            fn goal(&self) -> &'static str {
                "a thing"
            }
            fn checks(&self, _: &Workspace) -> Vec<Box<dyn Check>> {
                vec![Box::new(NoteCheck { notes: self.notes.clone(), add: self.add, text: self.text })]
            }
        }
        for mode in ["queued-turn", "queued-check", "hook"] {
            for text in ["Use the built-in renderer", "   "] {
                let b = bench();
                let mut ws = worked_on(&b);
                let mut worker = check_worker(&ws, false);
                let seen = Arc::default();
                let script = Scripted {
                    turns: (0..5).map(|_| turn(vec![Block::Text { text: "No files changed.".into() }], StopReason::EndTurn)).collect(),
                    seen: Arc::clone(&seen),
                };
                let model = NoteModel {
                    script,
                    notes: worker.notes.clone(),
                    add: mode != "queued-check",
                    claim: mode == "hook",
                    dir: notes::notes_dir(&ws.inbox_dir()),
                    text,
                };
                let slot = ModelSlot {
                    name: "Scripted".into(),
                    model: Box::new(model),
                    local: true,
                    effort: None,
                    max_turns: 10,
                    context_tokens: 0,
                    tools: Default::default(),
                };
                let api = ApiConversation::start(Arc::new(EngineHost(worker.engine.clone())), vec![slot], LoopConfig::default()).unwrap();
                let entry = crate::BackendEntry {
                    config: crate::BackendConfig {
                        name: "Scripted".into(),
                        kind: BackendKind::OpenaiCompatible,
                        model: "scripted".into(),
                        base_url: None,
                        ssh_host: None,
                        enabled: true,
                    },
                    options: Default::default(),
                };
                let mut opts = RunOptions::default();
                opts.kinds.register(Arc::new(NoteKind { notes: worker.notes.clone(), add: mode == "queued-check", text }));
                let error = worker.run(&mut ws, entry, Some(api), None, super::super::KeptHistory { in_mode: true, elsewhere: false }, &opts).unwrap_err();
                assert!(error.message.contains("two builder turns"), "{error:?}");
                let requests = seen.lock().unwrap();
                let expected = if text.trim().is_empty() {
                    2
                } else if mode == "hook" {
                    3
                } else {
                    4
                };
                assert_eq!(requests.len(), expected, "mode={mode}, text={text:?}");
                if !text.trim().is_empty() && mode != "hook" {
                    assert!(requests[2].last().unwrap().text().contains(text), "the queued instruction reaches the builder");
                }
                assert!(worker.notes.lock().unwrap().is_empty(), "each instruction is consumed once");
                assert!(!notes::was_read(&notes::notes_dir(&ws.inbox_dir()), 42));
                assert_eq!(BuildProgress::load(&ws.root).unwrap().unchanged_turns, 2);
                assert!(!try_it(&b, &ws).0);
                assert_eq!(prompts_given(&b), 0, "plain Try again still cannot reset the saved count");
            }
        }
    }

    /// #469: after the builder changed, the session the old one saved isn't handed to the new
    /// one, which starts from the brief; the same agent still carries on.
    #[test]
    fn another_agents_session_is_not_continued() {
        let b = bench();
        let mut ws = worked_on(&b);
        ws.manifest.agent_session_by = Some("claude-code".into());
        ws.save().unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(ok, "{feed:?}");
        let told = std::fs::read_to_string(&b.prompts).unwrap();
        assert!(told.contains("Their request") && !told.contains("Pick up where you left off"), "{told}");
        let m = Workspace::open(&ws.root).unwrap().manifest;
        assert_eq!((m.agent_session, m.agent_session_by.as_deref()), (None, Some("stand-in")));

        let b = bench();
        let mut ws = worked_on(&b);
        ws.manifest.agent_session_by = Some("stand-in".into());
        ws.save().unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(ok, "{feed:?}");
        assert!(std::fs::read_to_string(&b.prompts).unwrap().contains("Pick up where you left off"));
        assert_eq!(Workspace::open(&ws.root).unwrap().manifest.agent_session.as_deref(), Some("session-1"));
    }

    /// A resumable stand-in: records the session id the runner would continue.
    struct ResumingStandIn {
        seen: Arc<craft_anything_platform::sync::Mutex<Vec<Option<String>>>>,
    }

    impl crate::cli_agents::AgentAdapter for ResumingStandIn {
        fn ids(&self) -> &'static [&'static str] {
            &["stand-in"]
        }
        fn args(&self, cx: &crate::cli_agents::ArgsCx) -> Vec<String> {
            self.seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(cx.resume.map(str::to_string));
            vec![cx.prompt.to_string()]
        }
        fn streams_json(&self) -> bool {
            false
        }
        fn resumes(&self) -> bool {
            true
        }
    }

    /// R2.9: a session saved in one mode is never continued in the other — a Clean room build
    /// must not carry what a Deep one read — and one saved before the mode was kept counts as
    /// Deep, so only Deep continues it.
    #[test]
    fn a_session_from_another_mode_is_not_continued() {
        for (saved, deep, continued) in [
            (Some(true), true, true),
            (Some(true), false, false),
            (Some(false), false, true),
            (Some(false), true, false),
            (None, true, true),
            (None, false, false),
        ] {
            let b = bench();
            let mut ws = worked_on(&b);
            ws.manifest.agent_session_deep = saved;
            ws.manifest.capture_deep = Some(deep);
            ws.save().unwrap();
            std::fs::write(ws.capture_dir().join("README.md"), "# What we found in the Deep run\n\ninside evidence").unwrap();
            b.hub.engine.session().settings.deep_mode = deep;
            let seen = Arc::new(craft_anything_platform::sync::Mutex::new(Vec::new()));
            let mut adapters = crate::cli_agents::AgentAdapters::built_in();
            adapters.register(Arc::new(ResumingStandIn { seen: seen.clone() }));
            let (ok, feed) = try_with(&b, &ws, RunOptions { adapters, ..Default::default() });
            assert!(ok, "saved={saved:?}, deep={deep}: {feed:?}");
            let told = std::fs::read_to_string(&b.prompts).unwrap();
            assert_eq!(told.contains("Pick up where you left off"), continued, "saved={saved:?}, deep={deep}: {told}");
            assert_eq!(told.contains("Their request"), !continued, "a fresh start gets the brief: {told}");
            let readme = std::fs::read_to_string(ws.capture_dir().join("README.md")).unwrap();
            assert_eq!(readme.contains("What we found"), continued, "saved={saved:?}, deep={deep}: the Deep notes are replaced: {readme}");
            if !continued {
                assert!(readme.contains("# No target"), "saved={saved:?}, deep={deep}: the new study wrote its notes: {readme}");
            }
            let expected = continued.then_some("session-1");
            let seen = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(!seen.is_empty(), "saved={saved:?}, deep={deep}: the stand-in ran");
            assert!(seen.iter().all(|r| r.as_deref() == expected), "saved={saved:?}, deep={deep}: the runner got {seen:?}");
        }
    }

    /// R2.9: a saved Claude `--resume` or `--continue` that would reuse the project's Deep session
    /// is refused in Clean room; other agents' sessions, other ids, Deep runs and unrelated
    /// arguments are left alone.
    #[test]
    fn a_saved_deep_session_in_claude_arguments_is_refused_in_clean_room() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
        ws.manifest.agent_session = Some("deep".into());
        ws.manifest.agent_session_deep = Some(true);
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "deep"]), false).is_err());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume=deep"]), false).is_err());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["-r", "deep"]), false).is_err());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "other"]), false).is_ok());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "deep"]), true).is_ok());
        assert!(
            refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "other", "--resume", "deep"]), false).is_err(),
            "any saved resume is checked"
        );
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--continue"]), false).is_err());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["-c"]), false).is_err());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--continue"]), true).is_ok());
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "codex", &args(&["--resume", "deep"]), false).is_ok(), "only Claude's flag is checked");
        ws.manifest.agent_session_by = Some("codex".into());
        assert!(
            refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "deep"]), false).is_ok(),
            "another agent's session isn't Claude's"
        );
        ws.manifest.agent_session_by = None;
        ws.manifest.agent_session_deep = None;
        assert!(refuse_deep_session_reuse_in_clean_room(&ws, "claude-code", &args(&["--resume", "deep"]), false).is_err(), "a legacy session counts as Deep");
    }

    /// R2.9: the build path itself refuses a saved Deep resume in Clean room and allows it in
    /// Deep, so removing the guard from `agent_spec` fails this test.
    #[test]
    fn the_build_path_refuses_a_saved_deep_resume_in_clean_room() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
        ws.manifest.agent_session = Some("deep".into());
        ws.manifest.agent_session_by = Some("claude-code".into());
        ws.manifest.agent_session_deep = Some(true);
        ws.save().unwrap();
        let entry = crate::BackendEntry {
            config: crate::BackendConfig {
                name: "Claude".into(),
                kind: BackendKind::AgentCli,
                model: "claude-code".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            options: crate::BackendOptions { args: vec!["--resume".into(), "deep".into()], ..Default::default() },
        };
        let clean = check_worker(&ws, false);
        let err = clean.agent_spec(&ws, &entry, &RunOptions::default()).unwrap_err();
        assert!(err.message.contains("saved in Deep mode"), "{}", err.message);
        let deep = check_worker(&ws, true);
        assert!(deep.agent_spec(&ws, &entry, &RunOptions::default()).is_ok(), "a Deep run may resume its session");
    }

    /// Add the local API backend the transcript tests start.
    fn local_api(b: &Bench) {
        b.hub
            .add_backend(
                crate::BackendConfig {
                    name: "Local".into(),
                    kind: BackendKind::OpenaiCompatible,
                    model: "test-model".into(),
                    base_url: Some("http://127.0.0.1:0/v1".into()),
                    ssh_host: None,
                    enabled: true,
                },
                None,
            )
            .unwrap();
    }

    /// R2.9: a continuation keeps its API transcript, so the run can recognize completed Study
    /// work; a genuinely fresh build still starts a new conversation.
    #[test]
    fn an_api_continuation_keeps_its_transcript_and_a_fresh_build_drops_it() {
        let b = bench();
        local_api(&b);
        let ws = worked_on(&b);
        std::fs::write(ws.transcript_path(), b"{}").unwrap();
        let run = RebuildRun::start(&b.hub, Workspace::open(&ws.root).unwrap(), RunOptions { backend: Some("Local".into()), ..RunOptions::default() }).unwrap();
        assert!(ws.transcript_path().is_file(), "a continuation keeps the conversation");
        drop(run);

        let b = bench();
        local_api(&b);
        let fresh = Workspace::create(&b.projects, "fresh", "Build a notes app", TargetRef::None).unwrap();
        std::fs::write(fresh.transcript_path(), b"{}").unwrap();
        let run = RebuildRun::start(
            &b.hub,
            Workspace::open(&fresh.root).unwrap(),
            RunOptions { backend: Some("Local".into()), message: Some("Build a notes app".into()), ..RunOptions::default() },
        )
        .unwrap();
        assert!(!fresh.transcript_path().exists(), "a fresh build starts a new conversation");
        drop(run);
    }

    /// R2.9: a transcript from the other mode isn't "carrying on", so a no-message run still
    /// studies as a fresh one instead of skipping the Deep study. A follow-up message with a
    /// rejected transcript studies too, even when a capture from the other mode exists.
    #[test]
    fn an_api_transcript_from_the_other_mode_is_not_carried_on() {
        let mut opts = RunOptions::default();
        assert!(!super::super::is_carrying_on(&opts, false), "nothing is under way yet");
        assert!(super::super::is_carrying_on(&opts, true), "a transcript in this mode is carried on");
        opts.message = Some("change it".into());
        assert!(!super::super::is_carrying_on(&opts, true), "a new request isn't carrying on");
        opts.message = None;
        opts.continuing = true;
        assert!(super::super::is_carrying_on(&opts, true), "an explicit resume in this mode is");
        assert!(!super::super::is_carrying_on(&opts, false), "one in another mode is not");
    }

    /// R2.9: a follow-up with a conversation kept in the other mode studies even when a capture
    /// exists; one that resumed a conversation in this mode builds from a capture in this mode
    /// without studying again. A capture from the other mode studies again, whatever conversation
    /// is kept.
    #[test]
    fn a_rejected_transcript_studies_even_with_a_capture() {
        use super::super::{KeptHistory, starts_with_study};
        let opts = RunOptions::default();
        let none = KeptHistory::default();
        assert!(starts_with_study(&opts, none, true), "no conversation for this mode studies even with a capture in this mode");
        let mut follow_up = RunOptions { message: Some("change it".into()), ..Default::default() };
        assert!(!starts_with_study(&follow_up, none, true), "a fresh follow-up with a capture in this mode doesn't study");
        assert!(starts_with_study(&follow_up, KeptHistory { elsewhere: true, ..Default::default() }, true), "a conversation rejected for mode studies");
        assert!(!starts_with_study(&follow_up, KeptHistory { in_mode: true, ..Default::default() }, true), "a conversation in this mode carries on");
        assert!(starts_with_study(&opts, KeptHistory { in_mode: true, ..Default::default() }, false), "no capture to build from: study");
        assert!(starts_with_study(&follow_up, none, false), "a capture from the other mode studies even for a fresh follow-up");
        assert!(
            starts_with_study(&follow_up, KeptHistory { in_mode: true, ..Default::default() }, false),
            "a capture from the other mode studies even when the conversation is in this mode"
        );
        follow_up.message = None;
        assert!(
            !starts_with_study(&follow_up, KeptHistory { in_mode: true, ..Default::default() }, true),
            "a same-mode continuation studies only without a usable capture"
        );
    }

    /// R2.9: a capture a Deep study left must not be handed to a Clean room run, even when the
    /// conversation the project keeps is in Clean room. The run studies again and replaces it.
    #[test]
    fn a_capture_from_the_other_mode_is_studied_again() {
        let b = bench();
        b.hub.engine.session().settings.deep_mode = false;
        let target = b.projects.join("target.bin");
        std::fs::write(&target, b"\x7fELF not really").unwrap();
        let mut ws = Workspace::create(&b.projects, "mode", "Recreate this app", TargetRef::Local { path: target }).unwrap();
        // A Deep study's capture, and a conversation the project kept in Clean room.
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::write(ws.capture_dir().join("README.md"), "# What we found in the Deep run\n\nDecompiled notes.").unwrap();
        ws.manifest.capture_deep = Some(true);
        ws.manifest.agent_session = Some("clean-session".into());
        ws.manifest.agent_session_deep = Some(false);
        ws.manifest.stack = "Anything".into();
        ws.save().unwrap();
        let (ok, feed) = try_with(&b, &ws, RunOptions::default());
        let notes = std::fs::read_to_string(ws.capture_dir().join("README.md")).unwrap_or_default();
        assert!(notes.starts_with("# Clean room"), "the Deep capture must not be handed to Clean room: {notes}\n{feed:?}");
        assert!(!notes.contains("Decompiled notes"), "{notes}");
        assert_eq!(Workspace::open(&ws.root).unwrap().manifest.capture_deep, Some(false), "the Clean room study recorded its mode");
        let _ = ok;

        // The other way: a Clean room capture must not be handed to a Deep run either; Deep studies
        // again and records its own capture.
        let b = bench();
        let target = b.projects.join("target-deep.bin");
        std::fs::write(&target, b"\x7fELF not really").unwrap();
        let mut ws = Workspace::create(&b.projects, "mode-deep", "Recreate this app", TargetRef::Local { path: target }).unwrap();
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::write(ws.capture_dir().join("README.md"), "# Clean room\n\nNothing inside was read.").unwrap();
        ws.manifest.capture_deep = Some(false);
        ws.manifest.agent_session = Some("deep-session".into());
        ws.manifest.agent_session_deep = Some(true);
        ws.manifest.stack = "Anything".into();
        ws.save().unwrap();
        let (ok, feed) = try_with(&b, &ws, RunOptions::default());
        let notes = std::fs::read_to_string(ws.capture_dir().join("README.md")).unwrap_or_default();
        assert!(notes.starts_with("# What we found"), "the Clean room capture must not be handed to Deep: {notes}\n{feed:?}");
        assert_eq!(Workspace::open(&ws.root).unwrap().manifest.capture_deep, Some(true), "the Deep study recorded its mode");
        let _ = ok;
    }

    /// A capture with no recorded mode (a failed study, or one copied in) isn't usable: a run
    /// studies again. A recorded capture is usable only in its own mode.
    #[test]
    fn a_capture_with_no_recorded_mode_is_studied_again() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
        assert!(!super::super::capture_in_mode(&ws, false), "no capture yet");
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        assert!(!super::super::capture_in_mode(&ws, true), "a capture with no mode isn't trusted in Deep");
        assert!(!super::super::capture_in_mode(&ws, false), "nor in Clean room");
        ws.manifest.capture_deep = Some(true);
        assert!(super::super::capture_in_mode(&ws, true), "a Deep capture is usable in Deep");
        assert!(!super::super::capture_in_mode(&ws, false), "but not in Clean room");
        ws.manifest.capture_deep = Some(false);
        assert!(!super::super::capture_in_mode(&ws, true), "a Clean room capture isn't handed to Deep");
        assert!(super::super::capture_in_mode(&ws, false));
    }

    /// A saved session records the mode it ran in, and a lost one clears both.
    #[test]
    fn a_session_is_kept_with_the_mode_it_ran_in() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "x", "A thing", TargetRef::None).unwrap();
        Worker::remember_session(&mut ws, Some("s1".into()), false).unwrap();
        let m = Workspace::open(&ws.root).unwrap().manifest;
        assert_eq!((m.agent_session.as_deref(), m.agent_session_deep), (Some("s1"), Some(false)));
        Worker::remember_session(&mut ws, Some("s2".into()), true).unwrap();
        let m = Workspace::open(&ws.root).unwrap().manifest;
        assert_eq!((m.agent_session.as_deref(), m.agent_session_deep), (Some("s2"), Some(true)));
        Worker::remember_session(&mut ws, None, true).unwrap();
        let m = Workspace::open(&ws.root).unwrap().manifest;
        assert_eq!((m.agent_session, m.agent_session_deep), (None, None), "a lost session leaves no mode behind");
    }

    #[test]
    fn a_new_project_still_starts_from_the_top() {
        let b = bench();
        let ws = Workspace::create(&b.projects, "x", "A thing", TargetRef::None).unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(ok, "{feed:?}");
        let told = std::fs::read_to_string(&b.prompts).unwrap();
        assert!(!told.contains("Pick up where you left off"), "{told}");
    }

    #[test]
    fn a_check_that_could_not_run_gets_a_second_go_and_the_build_is_not_blamed() {
        let b = bench();
        let mut ws = worked_on(&b);
        std::fs::write(ws.app_dir().join("check-fails"), "1").unwrap();
        ws.manifest.awaiting_check = true;
        ws.save().unwrap();
        let (ok, _) = try_it(&b, &ws);
        assert!(ok, "one lost browser doesn't fail a finished build");

        // It keeps failing: the run stops, says the build is fine, and Try again only checks.
        let mut ws = worked_on(&b);
        ws.manifest.name = "y".into();
        ws.manifest.awaiting_check = false;
        ws.save().unwrap();
        std::fs::write(ws.app_dir().join("check-fails"), "2").unwrap();
        let (ok, feed) = try_it(&b, &ws);
        assert!(!ok);
        assert!(
            feed.iter().any(|i| matches!(i, feed::FeedItem::Error { message, .. } if message.contains("The build is finished, but it couldn't be checked"))),
            "{feed:?}"
        );
        let asked = prompts_given(&b);
        assert!(Workspace::open(&ws.root).unwrap().manifest.awaiting_check, "the builder's turn is on record");
        let (ok, _) = try_it(&b, &ws);
        assert!(ok);
        assert_eq!(prompts_given(&b), asked, "and it isn't asked to build again");
    }

    /// #736: a builder that says it built the site while app/ has no files stops after two
    /// unchanged turns, and the build check and the stop both say nothing was written. The
    /// reporter's release had no such stop, so its check ran 40+ times.
    #[test]
    fn an_empty_app_with_a_claimed_build_stops_and_says_nothing_was_written() {
        let b = bench();
        let mut ws = Workspace::create(&b.projects, "x", "A website for a bakery", TargetRef::None).unwrap();
        ws.manifest.agent_session = Some("session-1".into());
        ws.manifest.capture_deep = Some(true);
        ws.save().unwrap();
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::create_dir_all(ws.app_dir()).unwrap();
        // The stand-in answers "Done." and writes nothing, as the reported builder did.
        let opts = RunOptions { backend: Some("Stand-in".into()), ..RunOptions::default() };
        let run = RebuildRun::start(&b.hub, Workspace::open(&ws.root).unwrap(), opts).unwrap();
        let ok = loop {
            match run.rx.recv_timeout(Duration::from_secs(60)) {
                Ok(RunEvent::Finished { ok, .. }) => break ok,
                Ok(_) => {}
                Err(e) => panic!("the run went away: {e}"),
            }
        };
        assert!(!ok);
        assert_eq!(prompts_given(&b), 2, "a third unchanged turn would spend more without progress");
        let saved = Workspace::open(&ws.root).unwrap();
        let build = saved.manifest.checks.iter().find(|c| c.check == CheckResult::BUILD).expect("the build check ran");
        assert!(build.note.contains("app/ has no files yet, so the builder hasn't written the project."), "{}", build.note);
        let feed = feed::load(&saved);
        assert!(
            feed.iter().any(|i| matches!(i, feed::FeedItem::Error { message, .. }
                if message.contains("changed no files in app/") && message.contains("app/ has no files yet"))),
            "the stop says app/ is empty: {feed:?}"
        );
    }

    /// The empty-folder wording is only for an empty app/: a folder with files gets the plain stop.
    #[test]
    fn the_stop_names_an_empty_app_and_only_then() {
        let b = bench();
        let ws = worked_on(&b);
        let empty = no_build_progress(&ws);
        assert!(empty.message.contains("app/ has no files yet"), "{empty:?}");
        assert!(empty.remediation.as_deref().is_some_and(|r| r.contains("asks for the project's files in app/")), "{empty:?}");
        std::fs::write(ws.app_dir().join("index.html"), "<h1>Bakery</h1>").unwrap();
        let written = no_build_progress(&ws);
        assert!(!written.message.contains("has no files"), "{written:?}");
        assert_eq!(written.message, "The build stopped after two builder turns changed no files in app/.");
    }
}

#[test]
fn just_ask_is_told_to_the_builder_shortly_and_cut_on_a_character() {
    use craft_anything_engine::questions::AskedQuestion;
    assert_eq!(super::just_asked(&[]), "", "nothing asked, nothing added");
    let q = |n: usize, answer: String| AskedQuestion::new(format!("question {n}"), answer, true, "Claude".into(), None, 0);
    let mut asked: Vec<_> = (0..7).map(|n| q(n, "short".into())).collect();
    asked.push(q(7, "é".repeat(3000)));
    let told = super::just_asked(&asked);
    assert!(!told.contains("question 2") && told.contains("question 3") && told.contains("question 7"), "the latest five: {told}");
    assert!(told.contains(&format!("{}…", "é".repeat(1500))) && !told.contains(&"é".repeat(1501)), "a long answer is cut");
    assert!(told.contains("A: short\n"), "a short one isn't marked as cut");
    // The most a person can ask, five times over, still leaves room on a Windows command line, which
    // counts UTF-16 units: an emoji is two.
    let longest: Vec<_> = (0..5).map(|_| AskedQuestion::new("😀".repeat(4000), "😀".repeat(100_000), true, "Claude".into(), None, 0)).collect();
    assert!(super::just_asked(&longest).encode_utf16().count() < 20_000);
}

/// A run that ends by a caught bug must not leave the saved status as it last was ("Checking"
/// while nothing runs), so the project looks stuck. Every ending records a status.
#[test]
fn a_caught_bug_leaves_a_status_the_person_can_act_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(dir.path(), "x", "Build an app", TargetRef::None).unwrap();
    let panicked: Result<Result<String, ToolError>, Box<dyn std::any::Any + Send>> = Err(Box::new("boom"));
    let (ok, message, paused) = super::run_outcome(panicked, &mut ws);
    assert!(!ok && !paused && message.contains("hit a bug"), "{message}");
    let saved = Workspace::open(&ws.root).unwrap();
    assert_eq!(saved.manifest.status, Status::NeedsYou, "a project can't be left saying Checking with nothing running");
    assert!(saved.manifest.note.contains("hit a bug"), "{}", saved.manifest.note);
    // A pause is not a failure.
    let (ok, message, paused) = super::run_outcome(Ok(Err(ToolError::cancelled())), &mut ws);
    assert!(!ok && paused && message == "Paused.");
    assert_eq!(Workspace::open(&ws.root).unwrap().manifest.status, Status::Stopped);
    // A finished run keeps the status the run itself set.
    let (ok, message, paused) = super::run_outcome(Ok(Ok("Ready.".into())), &mut ws);
    assert!(ok && !paused && message == "Ready.");
}

/// #923: with OpenRouter as the builder, every asset call said "No fal.ai key" with one saved. The
/// project hasn't turned on generated assets, so nothing is sent or spent.
#[test]
fn a_saved_asset_key_gets_an_api_builders_tool_past_the_key_check() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::create(dir.path(), "x", "Make a game", TargetRef::None).unwrap();
    let mut worker = check_worker(&ws, false);
    worker.attach(&ws).unwrap();
    let call =
        |w: &Worker| w.engine.call("assets_image_generate", json!({"prompt": "a crate", "save_as": "crate.png"}), CallOptions::new(Caller::Ui)).err().unwrap();
    assert_eq!(call(&worker).code, ErrorCode::BadSettings, "no key loaded yet");
    let store = crate::KeyStore::file_only(dir.path().join("keys.toml"));
    store.set(crate::keys::FAL_KEY, "k-1").unwrap();
    worker.load_asset_keys(&store);
    assert_eq!(call(&worker).code, ErrorCode::NoPermission);
    assert!(worker.assets, "the brief and the tools go by the same read");
}

/// A worker over a test session and the workspace's feed, for check tests.
pub(crate) fn check_worker(ws: &Workspace, deep: bool) -> Worker {
    let mut s = craft_anything_engine::testing::session();
    s.settings.deep_mode = deep;
    let (tx, _rx) = std::sync::mpsc::channel();
    Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(s)),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(ws))),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
    }
}

#[cfg(unix)]
#[test]
fn console_study_keeps_two_real_observations_for_each_input() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let program = d.path().join("counter-tool");
    std::fs::write(
        &program,
        "#!/bin/sh\nn=0\nif [ -f run-count.txt ]; then read -r n < run-count.txt; fi\nn=$((n + 1))\nprintf '%s\\n' \"$n\" > run-count.txt\nprintf 'Result 42 completed in %s ms\\n' \"$n\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let ws = Workspace::create(d.path(), "Counter", "Rebuild this console tool", TargetRef::Local { path: program.clone() }).unwrap();
    let worker = check_worker(&ws, true);
    worker.capture_runs(&program, &ws.capture_dir()).unwrap();
    let records = kinds::runs::captured(&ws);
    assert_eq!(records.len(), kinds::runs::CASES.len());
    assert_eq!(std::fs::read_to_string(d.path().join("run-count.txt")).unwrap().trim(), "6");
    for record in records {
        assert_eq!(record["reason"], "exited");
        assert_eq!(record["repeat"]["reason"], "exited");
        assert_ne!(record["lines"], record["repeat"]["lines"]);
        let built = json!({"reason":"exited", "exit_code":0, "lines":["Result 42 completed in 999 ms"]});
        assert!(kinds::runs::run_claims(&record, &built, &[]).iter().all(|c| c.passed));
        let wrong = json!({"reason":"exited", "exit_code":0, "lines":["Result -42 completed in 999 ms"]});
        assert!(!kinds::runs::run_claims(&record, &wrong, &[])[1].passed);
    }
}

#[test]
fn a_check_that_worked_stands_while_nothing_it_looks_at_changes() {
    use craft_anything_engine::workspace::progress::CheckedBuild;
    use std::cell::Cell;
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    std::fs::create_dir_all(ws.app_dir().join("src")).unwrap();
    std::fs::write(ws.app_dir().join("src/main.rs"), "fn main() {}\n").unwrap();
    let w = check_worker(&ws, false);
    let runs = Cell::new(0);
    let worked = Cell::new(true);
    let check = |ws: &Workspace, round| {
        w.checks_unless_unchanged(ws, &SheetKind, round, || {
            runs.set(runs.get() + 1);
            Ok(vec![CheckResult::new(CheckResult::BUILD, round, worked.get(), "built")])
        })
        .unwrap()
    };

    assert_eq!(check(&ws, 1)[0].round, 1);
    assert_eq!(runs.get(), 1);
    // The builder's own check, then the one after its turn: nothing changed, so it isn't run again.
    let kept = check(&ws, self_check::SELF_CHECK_ROUND);
    assert_eq!((runs.get(), kept[0].round, kept[0].passed), (1, self_check::SELF_CHECK_ROUND, true));
    assert_eq!(check(&ws, 2)[0].round, 2);
    assert_eq!(runs.get(), 1);

    // A changed file, a new picture to compare with, or Deep mode checks again.
    std::fs::write(ws.app_dir().join("src/main.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
    check(&ws, 3);
    assert_eq!(runs.get(), 2);
    std::fs::create_dir_all(ws.reference_dir()).unwrap();
    std::fs::write(ws.reference_dir().join("look.png"), "a picture").unwrap();
    check(&ws, 4);
    assert_eq!(runs.get(), 3);
    let deep = check_worker(&ws, true);
    *deep.kept_checks.lock().unwrap_or_else(PoisonError::into_inner) = std::mem::take(&mut *w.kept_checks.lock().unwrap_or_else(PoisonError::into_inner));
    deep.checks_unless_unchanged(&ws, &SheetKind, 5, || {
        runs.set(runs.get() + 1);
        Ok(vec![CheckResult::new(CheckResult::BUILD, 5, true, "built")])
    })
    .unwrap();
    assert_eq!(runs.get(), 4, "Deep mode checks again");

    // A round with a failure is never kept: Try again looks again.
    worked.set(false);
    check(&ws, 6);
    check(&ws, 7);
    assert_eq!(runs.get(), 6);
    worked.set(true);

    // Kept across runs: the round saved in the progress record, while craft.json still ends with it.
    let round = check(&ws, 8);
    assert_eq!(runs.get(), 7);
    let saved = w.kept_checks.lock().unwrap_or_else(PoisonError::into_inner).saved_for(8);
    assert!(w.kept_checks.lock().unwrap_or_else(PoisonError::into_inner).saved_for(7).is_none());
    ws.manifest.checks.extend(round);
    let progress = BuildProgress { checked: saved.clone(), ..BuildProgress::default() };
    let next_run = check_worker(&ws, false);
    *next_run.kept_checks.lock().unwrap_or_else(PoisonError::into_inner) = kept_check::KeptChecks::from_saved(&progress, &ws);
    let again = next_run.checks_unless_unchanged(&ws, &SheetKind, 9, || panic!("an unchanged build isn't checked again")).unwrap();
    assert_eq!((again[0].round, again[0].note.as_str()), (9, "built"));
    // A copy of the project, with the same files, checks its own build.
    let other = tempfile::tempdir().unwrap();
    let copy = Workspace::create(other.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    std::fs::create_dir_all(copy.app_dir().join("src")).unwrap();
    std::fs::copy(ws.app_dir().join("src/main.rs"), copy.app_dir().join("src/main.rs")).unwrap();
    std::fs::create_dir_all(copy.reference_dir()).unwrap();
    std::fs::copy(ws.reference_dir().join("look.png"), copy.reference_dir().join("look.png")).unwrap();
    let copied = Cell::new(false);
    next_run
        .checks_unless_unchanged(&copy, &SheetKind, 10, || {
            copied.set(true);
            Ok(Vec::new())
        })
        .unwrap();
    assert!(copied.get(), "a copy is checked again");
    // A record naming another round than craft.json's last one keeps nothing.
    let stale = BuildProgress { checked: Some(CheckedBuild { round: 7, ..saved.unwrap() }), ..BuildProgress::default() };
    assert!(kept_check::KeptChecks::from_saved(&stale, &ws).saved_for(7).is_none());
}

/// A new Python project reaches the same build/screenshot path as a packaged app. The local
/// interpreter is a stand-in: no installed Python, packages, window, network or user settings.
#[cfg(unix)]
#[test]
fn new_python_projects_build_and_save_screenshots_through_their_toolchain() {
    use craft_anything_engine::tools::web_common::{Image, encode_png};
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::create(dir.path(), "Python sample", "Make an app in Python", TargetRef::None).unwrap();
    let app = ws.app_dir();
    std::fs::create_dir_all(app.join(".venv/bin")).unwrap();
    std::fs::write(app.join("main.py"), "# Owned interpreter fixture\n").unwrap();
    std::fs::write(app.join("pyproject.toml"), "[project]\nname = 'sample'\nversion = '0.1.0'\n").unwrap();
    // An HTML asset beside Python code must not bypass the Python build as a static site.
    std::fs::write(app.join("index.html"), "<h1>asset</h1>").unwrap();
    let program = app.join(".venv/bin/python");
    std::fs::write(
        &program,
        "#!/bin/sh\n\
        if [ \"$1\" = '-m' ]; then\n\
          printf '%s\\n' \"$@\" > build-args.txt\n\
          if [ -f fail-build ]; then echo 'error: fixture compilation failed'; exit 1; fi\n\
          exit 0\n\
        fi\n\
        [ \"$1\" = 'main.py' ] || exit 8\n\
        [ \"$2\" = '--screenshot' ] || exit 8\n\
        [ ! -f fail-shot ] || exit 9\n\
        cp fixture.png \"$3\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let rgba = (0..64 * 36).flat_map(|i| if i % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
    std::fs::write(app.join("fixture.png"), encode_png(&Image { width: 64, height: 36, rgba }).unwrap()).unwrap();
    let worker = check_worker(&ws, false);
    let checks = worker.run_checks(&ws, &AppKind, 1).unwrap();
    assert_eq!(checks.iter().map(|c| (c.check.as_str(), c.passed)).collect::<Vec<_>>(), [("build", true), ("app-screenshot", true)], "{checks:?}");
    let args = std::fs::read_to_string(app.join("build-args.txt")).unwrap();
    assert!(args.starts_with("-m\ncompileall\n-q\n") && args.ends_with(".\n"), "{args}");
    assert_eq!(args.lines().nth(3), Some("-f"), "{args}");
    assert_eq!(launch(&app, None).unwrap(), (program, vec!["main.py".into()]), "Play uses the same interpreter/entrypoint");
    let shot = ws.checks_dir().join("round-1/screenshot.png");
    assert!(shot.is_file());

    std::fs::write(app.join("fail-shot"), "").unwrap();
    let checks = worker.run_checks(&ws, &AppKind, 1).unwrap();
    assert!(checks[0].passed && !checks[1].passed && checks[1].note.contains("code 9"), "{checks:?}");
    assert!(!shot.exists(), "the old screenshot cannot make a failed run pass");
    std::fs::write(app.join("fail-build"), "").unwrap();
    let checks = worker.run_checks(&ws, &AppKind, 2).unwrap();
    assert_eq!(checks.len(), 1);
    assert!(!checks[0].passed && checks[0].note.contains("fixture compilation failed"), "{checks:?}");
}

/// Opt-in smoke test with a real local Python; all files and bytecode stay in the temp project.
/// The fixture writes a known nonblank PNG without opening a GUI or importing external packages.
#[cfg(unix)]
#[test]
#[ignore = "requires Python 3 on PATH; runs an owned headless fixture"]
fn real_python_new_project_build_and_screenshot() {
    use craft_anything_engine::tools::web_common::{Image, encode_png};
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::create(dir.path(), "Python smoke", "An app in Python", TargetRef::None).unwrap();
    let app = ws.app_dir();
    std::fs::create_dir_all(app.join(".venv/bin")).unwrap();
    let python = craft_anything_platform::which("python3").expect("install Python 3 to run this ignored test");
    std::os::unix::fs::symlink(python, app.join(".venv/bin/python")).unwrap();
    let rgba = (0..64 * 36).flat_map(|i| if i % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
    let png = encode_png(&Image { width: 64, height: 36, rgba }).unwrap();
    let source = format!("import pathlib, sys\nif sys.argv[1] == '--screenshot':\n    pathlib.Path(sys.argv[2]).write_bytes(bytes({png:?}))\n");
    std::fs::write(app.join("main.py"), source).unwrap();
    // compileall's cache check uses whole-second mtimes. Keep both revisions at the same
    // timestamp so a stale bytecode cache cannot make a syntax error look like a good build.
    let same_time = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    std::fs::File::open(app.join("main.py")).unwrap().set_modified(same_time).unwrap();
    std::fs::write(app.join(".venv/ignored.py"), "def broken(\n").unwrap();
    let worker = check_worker(&ws, false);
    let checks = worker.run_checks(&ws, &AppKind, 1).unwrap();
    assert_eq!(checks.iter().map(|c| (c.check.as_str(), c.passed)).collect::<Vec<_>>(), [("build", true), ("app-screenshot", true)], "{checks:?}");
    std::fs::write(app.join("main.py"), "def broken(\n").unwrap();
    std::fs::File::open(app.join("main.py")).unwrap().set_modified(same_time).unwrap();
    let checks = worker.run_checks(&ws, &AppKind, 2).unwrap();
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert!(!checks[0].passed && checks[0].note.contains("SyntaxError"), "{checks:?}");
}

/// A kind with its checks after the first `.1`, so a test needn't build or start anything.
struct LaterChecks(&'static dyn TargetKind, usize);

impl TargetKind for LaterChecks {
    fn id(&self) -> &'static str {
        self.0.id()
    }
    fn words(&self) -> KindWords {
        self.0.words()
    }
    fn detect(&self, _cx: &DetectCx) -> Option<u32> {
        None
    }
    fn default_stack(&self, cx: &DetectCx) -> &'static str {
        self.0.default_stack(cx)
    }
    fn goal(&self) -> &'static str {
        self.0.goal()
    }
    fn checks(&self, ws: &Workspace) -> Vec<Box<dyn Check>> {
        self.0.checks(ws).into_iter().skip(self.1).collect()
    }
}

/// The app's sheet only.
const APP_SHEET_ONLY: LaterChecks = LaterChecks(&AppKind, 3);

#[test]
fn apps_keep_the_sheet_only_when_deep_mode_decompiles_a_program() {
    let d = tempfile::tempdir().unwrap();
    let prog = Workspace::create(d.path(), "Tool", "Rebuild this app", TargetRef::Local { path: d.path().join("tool.exe") }).unwrap();
    let ids = AppKind.checks(&prog).iter().map(|c| c.id()).collect::<Vec<_>>();
    assert_eq!(ids, ["build", "app-runs", "app-screenshot", "mechanics"]);
    assert!(brief(&prog, "app", true, false, None, false, None).contains(kinds::mechanics::MECHANICS_CONTRACT));
    assert!(!brief(&prog, "app", false, false, None, false, None).contains("app/mechanics.json"), "Clean room doesn't ask for it");

    // Deep mode on a program: the sheet is checked (and missing here).
    let r = check_worker(&prog, true).run_checks(&prog, &APP_SHEET_ONLY, 1).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].check == "mechanics" && !r[0].passed, "{:?}", r[0]);
    // Clean room, or nothing to decompile: skipped, leaving no result.
    assert!(check_worker(&prog, false).run_checks(&prog, &APP_SHEET_ONLY, 1).unwrap().is_empty());
    let fresh = Workspace::create(d.path(), "Fresh", "A notes app", TargetRef::None).unwrap();
    assert!(check_worker(&fresh, true).run_checks(&fresh, &APP_SHEET_ONLY, 1).unwrap().is_empty());
}

#[test]
fn games_are_asked_for_the_sheet_checks_read() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::Local { path: d.path().to_path_buf() }).unwrap();
    let ids = |ws: &Workspace| GameKind.checks(ws).iter().map(|c| c.id()).collect::<Vec<_>>();
    assert_eq!(ids(&ws), ["build", "app-screenshot", "game-self-test", "mechanics", "credits"]);
    let b = brief(&ws, "game", false, false, None, false, None);
    assert!(b.contains("app/mechanics.json") && b.contains(kinds::mechanics::MECHANICS_CONTRACT), "{b}");
    let web = Workspace::create(d.path(), "Web game", "A game of this site", TargetRef::Website { url: "https://example.com".into() }).unwrap();
    assert_eq!(ids(&web), ["build", "app-screenshot", "game-self-test"], "a website's game has no sheet and no original's files");
    let web_brief = brief(&web, "game", true, true, None, false, None);
    assert!(web_brief.contains(kinds::self_test::SELF_TEST_CONTRACT) && !web_brief.contains("CREDITS.md"), "{web_brief}");
}

#[test]
fn a_game_is_ready_only_when_it_plays_and_credits_the_originals_files() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Caves", "Copy this game", TargetRef::Local { path: d.path().join("game.exe") }).unwrap();
    // The self-test needs a built game to start.
    let r = check_worker(&ws, false).run_checks(&ws, &LaterChecks(&GameKind, 2), 1).unwrap();
    assert!(r[0].check == "game-self-test" && !r[0].passed && r[0].note == "There's no built program to start yet.", "{:?}", r[0]);

    // Deep mode on a program: the original's files must be credited and the README says they're private.
    const CREDITS_ONLY: LaterChecks = LaterChecks(&GameKind, 4);
    let r = check_worker(&ws, true).run_checks(&ws, &CREDITS_ONLY, 1).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].check == "credits" && !r[0].passed && r[0].note.contains("app/CREDITS.md") && r[0].note.contains(PRIVATE_USE_NOTE), "{:?}", r[0]);
    std::fs::create_dir_all(ws.app_dir()).unwrap();
    std::fs::write(
        ws.app_dir().join("README.md"),
        format!(
            "# Caves

{PRIVATE_USE_NOTE}
"
        ),
    )
    .unwrap();
    std::fs::write(
        ws.app_dir().join("CREDITS.md"),
        "- Content/Rock.uasset from the game's Paks folder
",
    )
    .unwrap();
    assert!(check_worker(&ws, true).run_checks(&ws, &CREDITS_ONLY, 2).unwrap()[0].passed);
    // Clean room copies none of the original's files, and a game without a target has none: skipped.
    assert!(check_worker(&ws, false).run_checks(&ws, &CREDITS_ONLY, 1).unwrap().is_empty());
    let snake = Workspace::create(d.path(), "Snake", "A snake game", TargetRef::None).unwrap();
    assert!(check_worker(&snake, true).run_checks(&snake, &CREDITS_ONLY, 1).unwrap().is_empty());
}

#[test]
fn app_check_compares_the_labelled_controls() {
    use kinds::screenshot::controls_likeness;
    let tree = |nodes: &[(&str, &str)]| json!({"nodes": nodes.iter().map(|(role, title)| json!({"role": role, "title": title})).collect::<Vec<_>>()});
    let original =
        tree(&[("Window", "Character Map"), ("Pane", "Characters to copy :"), ("Button", "&Copy"), ("Button", "Select"), ("Image", ""), ("MenuItem", "File")]);
    // Case, spacing, mnemonics and a trailing colon don't matter; unlabelled parts aren't counted.
    let same =
        tree(&[("AXWindow", "character map"), ("AXStaticText", "Characters  to copy"), ("AXButton", "Copy"), ("AXButton", "Select"), ("AXMenuItem", "File")]);
    let c = controls_likeness(&original, &same).unwrap();
    assert_eq!((c.similarity.round(), c.missing.clone()), (100.0, Vec::<String>::new()));

    let thin = tree(&[("Window", "Character Map"), ("Button", "Select"), ("Button", "Help")]);
    let c = controls_likeness(&original, &thin).unwrap();
    assert_eq!(c.similarity.round(), 50.0, "2 shared of 5 + 3: 2*2/8");
    assert_eq!(c.missing, vec!["\"Characters to copy :\" (pane)", "\"Copy\" (button)", "\"File\" (menuitem)"]);
    assert_eq!(
        c.note(),
        "Its controls are 50% alike by their labels; the original also has \"Characters to copy :\" (pane), \"Copy\" (button), \"File\" (menuitem)."
    );
    assert!(controls_likeness(&tree(&[("Pane", "Game"), ("Pane", "")]), &thin).is_none(), "a game window has nothing to compare");

    // Each control a person uses is a requirement of its own; text labels stay advisory.
    let claims = kinds::screenshot::control_claims(&original, &thin);
    let got: Vec<(&str, bool)> = claims.iter().map(|c| (c.title.as_str(), c.passed)).collect();
    assert_eq!(
        got,
        [
            ("It has the original's \"Copy\" button", false),
            ("It has the original's \"Select\" button", true),
            ("It has the original's \"File\" menuitem", false)
        ]
    );
    assert!(kinds::screenshot::control_claims(&original, &same).iter().all(|c| c.passed), "labels match whatever the platform calls the role");
}

#[test]
fn website_check_compares_the_page_structure() {
    use kinds::website::structure_likeness;
    let page = |parts: &[(&str, Option<&str>)]| json!({"outline": parts.iter().map(|(tag, role)| match role { Some(r) => json!({"tag": tag, "role": r}), None => json!({"tag": tag}) }).collect::<Vec<_>>()});
    let original =
        page(&[("header", None), ("nav", None), ("h1", None), ("p", None), ("ul", None), ("ul", None), ("form", None), ("input", None), ("footer", None)]);
    // Roles count as their element; ul and ol are both lists; paragraphs and divs aren't parts.
    let same = page(&[
        ("div", Some("banner")),
        ("div", Some("navigation")),
        ("h1", None),
        ("ol", None),
        ("ul", None),
        ("form", None),
        ("textarea", None),
        ("div", Some("contentinfo")),
    ]);
    let s = structure_likeness(&original, &same).unwrap();
    assert_eq!((s.similarity.round(), s.missing.clone()), (100.0, vec![]), "{s:?}");

    let thin = page(&[("header", None), ("h1", None), ("ul", None), ("footer", None)]);
    let s = structure_likeness(&original, &thin).unwrap();
    assert_eq!(s.similarity.round(), 67.0, "4 of 8 parts in order: 2*4/12");
    assert_eq!(s.missing, vec![("navigation bar", 1), ("form", 1), ("list", 1), ("form field", 1)]);
    assert_eq!(s.note(), "structure 67% alike; missing parts: a navigation bar, a form, a list, a form field");
    let s = structure_likeness(&original, &page(&[])).unwrap();
    assert!(s.note().contains("2 lists"), "{}", s.note());
    assert!(structure_likeness(&page(&[("p", None)]), &thin).is_none(), "nothing to compare");
}

#[test]
fn website_check_pairs_opened_states_by_kind_and_label() {
    use kinds::website::pair_states;
    let original = json!({"states": [
        {"kind": "menu", "label": "Products", "file": "state-1.png"},
        {"kind": "dialog", "label": "Sign  up", "file": "state-2.png"},
        {"kind": "tab", "label": "Pricing", "file": "state-3.png"},
        {"kind": "menu", "label": "Went elsewhere", "changed": ["address changed"]}
    ]});
    let built = json!({"states": [
        {"kind": "dialog", "label": "sign up", "file": "state-1.png"},
        {"kind": "menu", "label": "Products", "file": "state-2.png"},
        {"kind": "section", "label": "Pricing", "file": "state-3.png"}
    ]});
    let pairs: Vec<(&str, Option<&str>)> =
        pair_states(&original, &built).into_iter().map(|(o, b)| (o["file"].as_str().unwrap(), b.map(|b| b["file"].as_str().unwrap()))).collect();
    assert_eq!(
        pairs,
        vec![("state-1.png", Some("state-2.png")), ("state-2.png", Some("state-1.png")), ("state-3.png", None)],
        "unpictured states are left out"
    );
    assert!(pair_states(&json!({"states": []}), &built).is_empty());
}

#[test]
fn deeper_pages_come_from_the_captured_pages_links() {
    use kinds::website::{Route, pick_deeper_routes};
    let start = "https://shop.example.com/";
    let have = vec![Route { path: "/products".into(), slug: "products".into(), url: "https://shop.example.com/products".into() }];
    let products_page = json!({"links": [
        {"href": "https://shop.example.com/products/"},
        {"href": "https://shop.example.com/products/chairs"},
        {"href": "https://shop.example.com/products/tables"},
        {"href": "https://shop.example.com/"},
        {"href": "https://elsewhere.example.org/x"},
        {"href": "https://shop.example.com/brochure.pdf"}
    ]});
    let about_page = json!({"links": [{"href": "https://shop.example.com/products/chairs"}, {"href": "https://shop.example.com/team"}]});
    let r = pick_deeper_routes(&[products_page.clone(), about_page], start, &have, 5);
    let paths: Vec<&str> = r.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["/products/chairs", "/products/tables", "/team"], "new pages only, once each, same site, no files");
    assert_eq!(pick_deeper_routes(&[products_page], start, &have, 1).len(), 1, "never more than there's room for");
}

#[test]
fn website_check_compares_the_text_and_names_what_is_missing() {
    use kinds::website::text_likeness;
    let original = json!({
        "text": "Simple pricing for teams. Plans start at 9 dollars. FAQ: can I cancel? Yes, anytime.",
        "headings": [{"level": 1, "text": "Simple pricing"}, {"level": 2, "text": "FAQ"}]
    });
    let built = json!({
        "text": "Simple pricing for teams. Plans start at 9 dollars.",
        "headings": [{"level": 1, "text": "Simple  Pricing"}]
    });
    let t = text_likeness(&original, &built).unwrap();
    assert!(t.similarity > 50.0 && t.similarity < 90.0, "{}", t.similarity);
    assert_eq!(t.missing_headings, vec!["FAQ".to_string()], "case and spacing don't matter");
    assert!(t.missing_words.contains(&"faq".to_string()) && t.missing_words.contains(&"cancel".to_string()), "{:?}", t.missing_words);
    assert!(t.note().starts_with("text ") && t.note().contains("missing headings: \"FAQ\""), "{}", t.note());
    let same = text_likeness(&original, &original).unwrap();
    assert!((same.similarity - 100.0).abs() < 1e-9 && same.missing_headings.is_empty() && same.missing_words.is_empty());
    assert!(text_likeness(&json!({"text": ""}), &built).is_none(), "nothing to compare");
}

#[test]
fn a_resumed_game_builder_hears_the_asset_choice_as_it_is_now() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Game", "A snake game", TargetRef::None).unwrap();
    let game = kind("game");
    fn cx(ws: &Workspace, assets: bool) -> BriefCx<'_> {
        BriefCx { ws, deep: false, ghidra: false, editor: None, assets, image_model: None, sees_pictures: true }
    }
    assert!(game.follow_up_sections(&cx(&ws, true)).is_empty(), "off and never used: nothing to say");
    // Turned on after the first build, with a key: the follow-up carries the asset note and budget.
    ws.set_person(|m| m.generate_assets = true).unwrap();
    let on = game.follow_up_sections(&cx(&ws, true));
    assert!(on[0].contains("assets_model_generate") && on[0].contains("$5.00"), "{on:?}");
    assert!(game.follow_up_sections(&cx(&ws, false)).is_empty(), "no key: not offered");
    // Turned off after some were made: say so.
    let rec = craft_anything_engine::workspace::AssetRecord { file: "a.glb".into(), kind: "model".into(), prompt: "a crate".into(), cost_usd: 0.5, at_ms: 1 };
    ws.record_asset(rec).unwrap();
    ws.set_person(|m| m.generate_assets = false).unwrap();
    assert!(game.follow_up_sections(&cx(&ws, true))[0].contains("now off"));
    assert!(kind("website").follow_up_sections(&cx(&ws, true)).is_empty());
}

#[test]
fn a_resumed_api_builder_waits_for_the_runs_check() {
    let app = kind("app");
    for prompt in [super::follow_up("bigger title", app.as_ref(), true), continue_prompt(app.as_ref(), true)] {
        assert!(prompt.contains("Craft Anything checks your build after your turn"), "{prompt}");
        assert!(!prompt.contains("rebuild_check"), "{prompt}");
    }
}

#[test]
fn a_stopped_build_is_told_to_carry_on_not_start_over() {
    let p = continue_prompt(kind("website").as_ref(), false);
    assert!(p.contains("stopped before it finished") && p.contains("Pick up where you left off") && p.contains("app/"));
}

#[test]
fn the_extractor_takes_the_originals_out_and_the_builder_is_told_where() {
    let b = extract_brief(Path::new("D:/Games/Cave Game"));
    assert!(b.contains("D:/Games/Cave Game") && b.contains("app/Original/") && b.contains("permission: true"), "{b}");
    assert!(b.contains("Never decrypt") && b.contains("You don't build the game") && b.contains(&format!("app/{ORIGINALS_MANIFEST}")), "{b}");
    assert!(b.contains("never write your own archive reader") && b.contains("ending in `/`"), "{b}");
    assert!(ORIGINALS_READY.contains("app/Original/MANIFEST.md") && ORIGINALS_READY.contains("CREDITS.md"));
}

#[test]
fn the_check_names_padding_colour_font_and_width_differences() {
    use kinds::website::design_notes;
    let section = |heading: &str, pad: [u64; 4], per_row: u64, column: u64| json!({"tag": "section", "heading": heading, "pad": pad, "per_row": per_row, "column": {"max_w": column, "x": 80}});
    let original = json!({
        "design": {"backgrounds": [{"value": "#12181f"}, {"value": "#0c2a2a"}], "text_colors": [{"value": "#e6ecf2"}],
                   "fonts": [{"value": "Figtree, sans-serif"}], "font_sizes": [{"value": "64px"}, {"value": "16px"}],
                   "spacing": [{"value": "96px"}, {"value": "24px"}]},
        "layouts": [
            {"width": 1440, "page_w": 1440, "type": {"h1": 64, "body": 16}, "blocks": [json!({"tag": "header", "pad": [16, 32, 16, 32], "per_row": 1}), section("Plans", [96, 32, 96, 32], 3, 1280)]},
            {"width": 390, "page_w": 390, "type": {"h1": 40}, "blocks": [section("Plans", [64, 16, 64, 16], 1, 1280)]}
        ]
    });
    // next/font renames the family and the browser rounds a colour by a step: both still match.
    let same = json!({
        "design": {"backgrounds": [{"value": "#12181e"}, {"value": "#0c2a2a"}], "text_colors": [{"value": "#e6ecf2"}],
                   "fonts": [{"value": "\"__Figtree_a1b2c3\", \"__Figtree_Fallback_a1b2c3\""}], "font_sizes": [{"value": "64px"}, {"value": "16px"}],
                   "spacing": [{"value": "24px"}, {"value": "96px"}]},
        "layouts": original["layouts"].clone()
    });
    assert!(design_notes(&original, &same).is_empty(), "{:?}", design_notes(&original, &same));
    let built = json!({
        "design": {"backgrounds": [{"value": "#ffffff"}, {"value": "#0c2a2a"}], "text_colors": [{"value": "#e6ecf2"}],
                   "fonts": [{"value": "Inter, sans-serif"}], "font_sizes": [{"value": "48px"}, {"value": "16px"}], "spacing": [{"value": "24px"}]},
        "layouts": [
            {"width": 1440, "page_w": 1440, "type": {"h1": 48, "body": 16}, "blocks": [json!({"tag": "header", "pad": [16, 32, 16, 32], "per_row": 1}), section("Plans", [48, 24, 48, 24], 1, 1200)]},
            {"width": 390, "page_w": 512, "type": {"h1": 40}, "blocks": [json!({"tag": "section", "heading": "plans ", "pad": [64, 16, 64, 16], "per_row": 1})]}
        ]
    });
    let notes = design_notes(&original, &built);
    assert_eq!(notes[0], "colours the original uses that the build doesn't: #12181f (background)");
    assert_eq!(notes[1], "main font: Figtree in the original, Inter in the build");
    assert_eq!(notes[2], "text sizes the original uses that the build doesn't: 64px");
    assert_eq!(notes[3], "spacing the original uses that the build doesn't: 96px");
    assert_eq!(
        notes[4],
        "at 1440 px: top heading 48 px, the original's 64 px; the \"Plans\" section: padding 48 24 48 24 vs the original's 96 32 96 32, 1 per row vs 3, content column 1200 px vs 1280 px"
    );
    assert_eq!(notes[5], "at 390 px: the page scrolls sideways (512 px wide); the \"Plans\" section: no content column (the original's is 1280 px)");
    assert_eq!(notes.len(), 6);
    assert!(design_notes(&json!({}), &json!({})).is_empty(), "nothing to compare");
}

#[test]
fn menus_are_studied_on_a_phone_too() {
    use kinds::website::{STATES_DIR, phone_states_dir, phone_width, pictured_states, state_dirs};
    assert_eq!(phone_width(&[1440, 768, 390]), Some(390));
    assert_eq!(phone_width(&[1440, 1024]), None);
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Shop", "Clone it", TargetRef::Website { url: "https://shop.example.com/".into() }).unwrap();
    for (dir, width, label) in [(STATES_DIR.to_string(), 1440, "Products"), (phone_states_dir(390), 390, "Menu")] {
        let states = ws.capture_dir().join(dir);
        std::fs::create_dir_all(&states).unwrap();
        let list = json!({"width": width, "states": [{"kind": "menu", "label": label, "file": "state-1.png"}]});
        std::fs::write(states.join("states.json"), serde_json::to_vec(&list).unwrap()).unwrap();
    }
    std::fs::create_dir_all(ws.capture_dir().join("states-old")).unwrap();
    assert_eq!(state_dirs(&ws.capture_dir()), ["states", "states-390"]);
    assert_eq!(pictured_states(&ws), ["states/state-1.png (Products, menu, 1440 px)", "states-390/state-1.png (Menu, menu, 390 px)"]);
}

/// A builder whose turn lasts until its own check (asked like `rebuild_check` from another process)
/// has been answered by the run, then ends with that answer as its reply.
struct ChecksItsWork {
    root: PathBuf,
    asked: Option<std::thread::JoinHandle<Option<String>>>,
    reply: Option<String>,
}

impl Conversation for ChecksItsWork {
    fn send(&mut self, _text: &str) {
        let asks = Inbox::new(self_check::asks_dir(&self.root));
        self.asked = Some(std::thread::spawn(move || {
            let id = asks.post(craft_anything_engine::inbox::RequestKind::Question, "Check the build", "", Vec::new(), false, None).ok()?;
            asks.wait(&id, Duration::from_secs(20), &|| false).and_then(|a| a.text)
        }));
    }
    fn poll(&mut self) -> Vec<AgentEvent> {
        if self.asked.as_ref().is_some_and(|h| h.is_finished()) {
            self.reply = self.asked.take().and_then(|h| h.join().ok()).flatten();
        }
        match self.reply.take() {
            Some(text) => {
                vec![
                    AgentEvent::TextDelta { text },
                    AgentEvent::TurnDone { backend: "test".into(), input_tokens: 0, output_tokens: 0, cost_usd: None, cost_is_estimate: false },
                ]
            }
            None => Vec::new(),
        }
    }
    fn answer_consent(&mut self, _call_id: &str, _allow: bool) {}
    fn stop(&mut self) {}
    fn is_busy(&self) -> bool {
        true
    }
}

#[test]
fn preparing_the_builders_agent_does_not_lock_up() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let entry = crate::BackendEntry {
        config: crate::BackendConfig {
            name: "x".into(),
            kind: crate::BackendKind::AgentCli,
            model: "claude-code".into(),
            base_url: None,
            ssh_host: None,
            enabled: true,
        },
        options: Default::default(),
    };
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(w.agent_spec(&ws, &entry, &RunOptions::default()).unwrap().logs);
    });
    let logs = finished.recv_timeout(std::time::Duration::from_secs(20)).expect("it took the session lock twice and waited on itself");
    assert!(logs.is_some());
}

/// The builder's own `rebuild_check`, asked mid-turn, is answered by the run and isn't a round.
#[test]
fn the_run_answers_the_builders_own_check_during_its_turn() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: Some((Arc::new(SheetKind), 90.0)),
        reset_progress_after_notes: false,
    };
    let mut builder = ChecksItsWork { root: ws.root.clone(), asked: None, reply: None };
    let reply = w.turn(&mut ws, &mut builder, "Build it.", true, None).unwrap();
    // The same check the run makes after a turn, as round 0, with what to do next.
    assert!(reply.starts_with("mechanics: failed") && reply.contains("no app/mechanics.json") && reply.contains("check again with rebuild_check"), "{reply}");
    assert!(ws.manifest.checks.is_empty(), "the builder's own check isn't a round");
    let steps: Vec<String> = rx.try_iter().filter_map(|e| if let RunEvent::Step { text, .. } = e { Some(text) } else { None }).collect();
    assert!(steps.iter().any(|s| s.contains("checking its work")) && steps.iter().any(|s| s.contains("checked its work")), "the person sees it: {steps:?}");
    assert!(Inbox::new(self_check::asks_dir(&ws.root)).pending().is_empty(), "nothing left waiting");
    let left: Vec<_> = std::fs::read_dir(self_check::asks_dir(&ws.root)).unwrap().flatten().map(|e| e.file_name()).collect();
    // The run leaves its mark for the asker, which reads it after its wait and removes it: an
    // answer landing as the wait ends can't make the check look untaken.
    assert!(left.iter().any(|n| n.to_string_lossy().ends_with(".taken")), "the run marks the check it took: {left:?}");
}

/// A build stopped with a check taken answers it, so the builder isn't left waiting on a check
/// nothing runs any more; the run's own error stays the stop.
#[test]
fn a_stopped_build_answers_the_check_it_took() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Arena", "Recreate this game", TargetRef::None).unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: Some((Arc::new(SheetKind), 90.0)),
        reset_progress_after_notes: false,
    };
    let asks = Inbox::new(self_check::asks_dir(&ws.root));
    let id = asks.post(craft_anything_engine::inbox::RequestKind::Question, "Check the build", "", Vec::new(), false, None).unwrap();
    w.stop.cancel();
    let err = w.answer_self_checks(&mut ws).unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled, "the run still stops");
    let answer = asks.wait(&id, Duration::from_secs(2), &|| false).expect("the taken check is answered");
    assert_eq!(answer.text.as_deref(), Some(self_check::STOPPED_ANSWER));
    assert!(self_check::taken_mark(&ws.root, &id).unwrap().is_file(), "the mark is left for the asker to read");
}

/// A builder that reads one decompiled function (by its MCP name, as Claude Code calls it) and ends
/// its turn.
struct Decompiles {
    events: Vec<AgentEvent>,
}

impl Conversation for Decompiles {
    fn send(&mut self, _text: &str) {
        let tool = format!("mcp__craft-anything__{}", craft_anything_engine::tools::native_function::PSEUDOCODE_TOOL);
        self.events = vec![
            AgentEvent::ToolStarted { call_id: "1".into(), tool, input: json!({"function": "entry"}) },
            AgentEvent::TextDelta { text: "Read it.".into() },
            AgentEvent::TurnDone { backend: "test".into(), input_tokens: 0, output_tokens: 0, cost_usd: None, cost_is_estimate: false },
        ];
    }
    fn poll(&mut self) -> Vec<AgentEvent> {
        std::mem::take(&mut self.events)
    }
    fn answer_consent(&mut self, _call_id: &str, _allow: bool) {}
    fn stop(&mut self) {}
    fn is_busy(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Every request from the person is a new run: decompiling in an earlier one is kept with the
/// project, so Deep mode doesn't tell the builder it hasn't decompiled after each request.
#[test]
fn decompiling_once_is_remembered_by_the_project() {
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Sim", "Recreate this game", TargetRef::None).unwrap();
    assert!(!ws.manifest.decompiled);
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let mut builder = Decompiles { events: Vec::new() };
    w.turn(&mut ws, &mut builder, "Build it.", true, None).unwrap();
    assert!(ws.manifest.decompiled, "the decompile is seen by its MCP name");
    ws.save().unwrap();
    assert!(Workspace::open(&ws.root).unwrap().manifest.decompiled, "a later run reads it from craft.json");
}

/// A later request's run, reading the project from disk, reminds only a builder that never
/// decompiled in it, and only in Deep mode on a program.
#[test]
fn a_later_request_is_reminded_only_when_nothing_was_decompiled() {
    let d = tempfile::tempdir().unwrap();
    let program = TargetRef::Local { path: d.path().join("game.exe") };
    let mut ws = Workspace::create(d.path(), "Sim", "Recreate this game", program).unwrap();
    let reopened = |ws: &Workspace| Workspace::open(&ws.root).unwrap().manifest;
    assert!(needs_decompile_reminder(true, &reopened(&ws)), "nothing decompiled yet");
    assert!(!needs_decompile_reminder(false, &reopened(&ws)), "Clean room never decompiles");
    ws.manifest.decompiled = true;
    ws.save().unwrap();
    assert!(!needs_decompile_reminder(true, &reopened(&ws)), "decompiled in an earlier run");
    let none = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    assert!(!needs_decompile_reminder(true, &none.manifest), "no program to decompile");
}

#[test]
fn every_kind_leads_tests_its_own_build_and_keeps_its_unknowns() {
    let d = tempfile::tempdir().unwrap();
    let web = TargetRef::Website { url: "https://example.com".into() };
    for (k, target, stack) in [("website", web, stacks::BUILDERS_CHOICE), ("app", TargetRef::None, "Rust + egui (eframe)"), ("game", TargetRef::None, "Godot")]
    {
        let mut ws = Workspace::create(d.path(), k, "Rebuild it", target).unwrap();
        ws.manifest.stack = stack.into();
        let b = brief(&ws, k, false, false, None, false, None);
        assert!(b.contains("you lead the investigation") && b.contains("Don't repeat a call that already answered"), "{k}: {b}");
        assert!(b.contains("core_unknown_add") && b.contains("core_unknown_edit") && b.contains("Missing evidence is never proof"), "{k}");
        assert!(b.contains("go through your checklist and core_unknowns_show"), "{k}");
        assert!(b.contains("Test it against the original with rebuild_check") && b.contains("Before you reply, go through your checklist"), "{k}");
        if k == "website" {
            assert!(b.contains("Work from these pictures and measurements.\n\nMatch it exactly"), "one clean paragraph break: {b}");
            let deep = brief(&ws, k, true, false, None, false, None);
            assert!(deep.contains("(the rendered HTML).\n\nMatch it exactly"), "one clean paragraph break: {deep}");
        }
    }
    assert!(follow_up("bigger title", "game").contains("Check it with rebuild_check"));
    assert!(continue_prompt(kind("app").as_ref(), false).contains("Check it with rebuild_check"));
}

#[test]
fn the_check_names_errors_failed_requests_and_accessibility_the_build_adds() {
    use kinds::website::health_notes;
    let original = json!({
        "health": {"errors": [{"kind": "script", "text": "ads.js blocked"}], "failed_requests": [{"url": "https://ex.com/ads/pixel.gif", "status": 404}]},
        "a11y": {"lang": "en", "unnamed_controls": 1, "images_without_alt": 0}
    });
    // The original's own problems, served from elsewhere, aren't the build's to fix.
    let same = json!({
        "health": {"errors": [{"kind": "script", "text": "ads.js blocked"}], "failed_requests": [{"url": "http://127.0.0.1:5173/ads/pixel.gif", "status": 404}]},
        "a11y": {"lang": "en", "unnamed_controls": 1, "images_without_alt": 0}
    });
    assert!(health_notes(&original, &same).is_empty(), "{:?}", health_notes(&original, &same));
    let built = json!({
        "health": {
            "errors": [{"kind": "script", "text": "TypeError: x is undefined"}, {"kind": "console", "text": "Hydration failed"}],
            "failed_requests": [{"url": "http://127.0.0.1:5173/fonts/Figtree.woff2", "error": "net::ERR_FILE_NOT_FOUND"}, {"url": "http://127.0.0.1:5173/logo.png", "status": 404}]
        },
        "a11y": {"lang": "", "unnamed_controls": 4, "images_without_alt": 2}
    });
    assert_eq!(
        health_notes(&original, &built),
        [
            "the build logs 2 error(s) the original doesn't: \"TypeError: x is undefined\"; \"Hydration failed\"",
            "2 request(s) fail in the build: /fonts/Figtree.woff2 (net::ERR_FILE_NOT_FOUND), /logo.png (404)",
            "4 controls without a name (the original has 1)",
            "2 pictures without alt text (the original has 0)",
            "the page language isn't set (the original's is en)",
        ]
    );
    assert!(health_notes(&json!({}), &json!({})).is_empty(), "older captures have nothing to compare");
}

#[test]
fn each_thing_a_page_must_do_is_a_claim_of_its_own() {
    use kinds::website::page_claims;
    let original = json!({
        "layouts": [{"width": 1440, "page_w": 1440}, {"width": 390, "page_w": 390}, {"width": 320, "page_w": 400}],
        "health": {"errors": [], "failed_requests": []},
        "a11y": {"landmarks": {"banner": 1, "navigation": 1, "main": 1, "contentinfo": 1}}
    });
    let good = json!({
        "layouts": [{"width": 1440, "page_w": 1440}, {"width": 390, "page_w": 390}, {"width": 320, "page_w": 500}],
        "health": {"errors": [], "failed_requests": []},
        "a11y": {"landmarks": {"banner": 2, "navigation": 1, "main": 1, "contentinfo": 1}}
    });
    let claims = page_claims("The start page", &original, &good);
    let titles: Vec<&str> = claims.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "The start page doesn't scroll sideways at 1440 px",
            "The start page doesn't scroll sideways at 390 px",
            "The start page logs no errors the original doesn't",
            "The start page loads every file it asks for",
            "The start page has the original's landmarks",
        ],
        "no claim at a width where the original scrolls sideways too"
    );
    assert!(claims.iter().all(|c| c.passed));
    let broken = json!({
        "layouts": [{"width": 1440, "page_w": 1440}, {"width": 390, "page_w": 512}],
        "health": {"errors": [{"text": "TypeError: x"}], "failed_requests": [{"url": "http://127.0.0.1:1/logo.png", "status": 404}]},
        "a11y": {"landmarks": {"main": 1}}
    });
    let failed: Vec<(String, String)> = page_claims("/about", &original, &broken).into_iter().filter(|c| !c.passed).map(|c| (c.title, c.detail)).collect();
    assert_eq!(
        failed,
        [
            ("/about doesn't scroll sideways at 390 px".to_string(), "it's 512 px wide".to_string()),
            ("/about logs no errors the original doesn't".to_string(), "TypeError: x".to_string()),
            ("/about loads every file it asks for".to_string(), "/logo.png (404)".to_string()),
            ("/about has the original's landmarks".to_string(), "missing: banner, contentinfo, navigation".to_string()),
        ]
    );
    assert!(page_claims("x", &json!({}), &json!({})).is_empty(), "older captures make no claims");
}

#[test]
fn a_console_original_is_compared_by_its_runs_not_pictured() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Tool", "Rebuild this tool", TargetRef::Local { path: d.path().join("tool.exe") }).unwrap();
    let b = brief(&ws, "app", false, false, None, false, None);
    assert!(b.contains("--screenshot") && !b.contains("capture/runs/"), "a windowed app is pictured");
    let runs = ws.capture_dir().join(kinds::runs::RUNS_DIR);
    std::fs::create_dir_all(&runs).unwrap();
    std::fs::write(
        runs.join("help.json"),
        br#"{"name": "--help", "arguments": ["--help"], "reason": "exited", "exit_code": 0, "lines": ["Usage: tool <file>"]}"#,
    )
    .unwrap();
    let b = brief(&ws, "app", false, false, None, false, None);
    assert!(b.contains(kinds::RUNS_BRIEF) && !b.contains("Give the program a `--screenshot"), "{b}");
    assert_eq!(kinds::runs::captured(&ws).len(), 1, "the runs check applies, the picture check doesn't");
}

#[test]
fn deep_mode_hands_the_builder_every_tool_and_clean_room_none_that_look_inside() {
    let d = tempfile::tempdir().unwrap();
    let web = Workspace::create(d.path(), "Site", "Clone it", TargetRef::Website { url: "https://example.com".into() }).unwrap();
    let b = brief(&web, "website", true, false, None, false, None);
    assert!(b.contains("every craft-anything tool is yours") && b.contains("capture/how-it-works.md"), "{b}");
    assert!(b.contains("web_listeners_list") && b.contains("web_coverage_measure") && b.contains("js_sources_restore"), "{b}");
    assert!(b.contains("never paste the original's code into app/") && !b.contains("enable_toolset with"), "{b}");
    let clean = brief(&web, "website", false, false, None, false, None);
    assert!(clean.contains("what a visitor of the live site sees") && clean.contains("don't open or fetch the original site"), "{clean}");
    for deep_only in ["web_page_show", "web_listeners_list", "page.html", "breakpoints", "how-it-works"] {
        assert!(!clean.contains(deep_only), "{deep_only} in a Clean room brief: {clean}");
    }
    assert!(clean.contains("never open, read, list, fetch, run or decompile its files"), "{clean}");
    let prog = Workspace::create(d.path(), "Tool", "Rebuild it", TargetRef::Local { path: d.path().join("tool.exe") }).unwrap();
    let b = brief(&prog, "app", true, false, None, false, None);
    assert!(
        b.contains("core_target_triage")
            && b.contains("native_function_pseudocode")
            && b.contains("managed_assembly_show")
            && !b.contains("web_listeners_list"),
        "{b}"
    );
    let clean = brief(&prog, "app", false, false, None, false, None);
    assert!(!clean.contains("core_target_triage") && !clean.contains("native_") && clean.contains("ask the person with core_person_ask"), "{clean}");
}

#[test]
fn a_console_original_is_rebuilt_as_a_console_program() {
    let d = tempfile::tempdir().unwrap();
    let (tool, app) = (d.path().join("tool.exe"), d.path().join("app.exe"));
    std::fs::write(&tool, craft_anything_testkit::pe64_subsystem(3)).unwrap();
    std::fs::write(&app, craft_anything_testkit::pe64_subsystem(2)).unwrap();
    assert_eq!(choose("Rebuild this tool", &TargetRef::Local { path: tool }), ("app", "Rust (console)"));
    assert_eq!(choose("Rebuild this tool", &TargetRef::Local { path: app }).1, "Rust + egui (eframe)", "a windowed program stays a windowed app");
    let steps = stacks::steps("Rust (console)", None);
    assert!(steps.contains("console program") && steps.contains("exit codes") && steps.contains("draw no window"), "{steps}");
    assert_eq!(choose("A notes tool", &TargetRef::None).1, "Rust + egui (eframe)");
}

#[test]
fn a_note_the_builder_took_at_a_step_joins_the_feed_and_isnt_sent_again() {
    use craft_anything_engine::questions::{self, AskedQuestion};
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Site", "Clone it", TargetRef::None).unwrap();
    let asked = AskedQuestion { deep: false, ..AskedQuestion::new("where is the score kept".into(), "In src.".into(), true, "Stand-in".into(), None, 0) };
    questions::add(&ws, asked.clone()).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::new(Mutex::new(vec![
            Note { id: 1, text: "make the header blue".into(), shown: "make the header blue".into(), pictures: Vec::new(), questions: vec![asked] },
            Note { id: 2, text: "add a footer".into(), shown: "add a footer".into(), pictures: Vec::new(), questions: Vec::new() },
        ])),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let dir = notes::notes_dir(&ws.inbox_dir());
    notes::hand_over(&dir, 1, "make the header blue", None).unwrap();
    notes::hand_over(&dir, 2, "add a footer", None).unwrap();
    // The agent's hook takes the first one at a step; the second comes in after it.
    std::fs::rename(dir.join("2.json"), dir.join("2.wait")).unwrap();
    assert_eq!(notes::claim(&dir), ["make the header blue"]);
    std::fs::rename(dir.join("2.wait"), dir.join("2.json")).unwrap();
    w.relay_read_notes(&ws);
    assert!(questions::untold(&ws, false).is_empty(), "the answers in a read note are told");
    let shown: Vec<String> = rx.try_iter().filter_map(|e| if let RunEvent::Note { shown, .. } = e { Some(shown) } else { None }).collect();
    assert_eq!(shown, ["make the header blue"], "it joins the feed as soon as the builder took it");
    assert!(!notes::was_read(&dir, 1), "its files are gone");
    // After the turn, only the note nobody took is sent, and no hook can hand it over again.
    let (follow, _) = w.added(&mut ws, true, true).unwrap();
    assert!(follow.contains("add a footer") && !follow.contains("make the header blue"), "{follow}");
    assert!(notes::claim(&dir).is_empty());
}

/// From a live Next.js + shadcn/ui clone of a fixture page (R2.3): the builder writes its own
/// headings, so sections pair by tag in page order when the headings differ, and a nav's link
/// count isn't a grid's items per row.
#[test]
fn design_notes_pair_renamed_sections_and_leave_nav_links_alone() {
    use kinds::website::design_notes;
    let block = |tag: &str, heading: &str, pad: u64, per_row: u64| json!({"tag": tag, "heading": heading, "pad": [pad, 0, pad, 0], "per_row": per_row, "column": {"max_w": 1100}});
    let layout = |blocks: Vec<Value>| json!({"layouts": [{"width": 768, "page_w": 753, "type": {"h1": 36}, "blocks": blocks}]});
    let original = layout(vec![block("nav", "", 0, 2), block("section", "Why people switch", 48, 2), block("section", "Plans", 48, 1)]);
    let same_but_renamed = layout(vec![block("nav", "", 0, 3), block("section", "Why people like it", 48, 2), block("section", "Plans", 48, 1)]);
    assert!(design_notes(&original, &same_but_renamed).is_empty(), "{:?}", design_notes(&original, &same_but_renamed));
    let tighter = layout(vec![block("nav", "", 0, 3), block("section", "Why people like it", 24, 2), block("section", "Plans", 48, 1)]);
    let notes = design_notes(&original, &tighter);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("\"Why people switch\" section: padding 24 0 24 0 vs the original's 48 0 48 0"), "{notes:?}");
}

#[test]
fn a_project_in_a_picked_folder_tells_the_builder_what_its_files_are() {
    use craft_anything_engine::workspace::ExistingFiles;
    let d = tempfile::tempdir().unwrap();
    let settings = craft_anything_engine::Settings::default();
    let folder = |name: &str| {
        let f = d.path().join(name);
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("index.html"), "<p>hi</p>").unwrap();
        f
    };
    let usual = project::new_project(&settings, "-", "A notes app", project::Place::In(d.path()), None, false, None).unwrap();
    assert!(!brief(&usual, "app", false, false, None, false, None).contains("folder of their own"), "a usual project says nothing about it");

    let mine = folder("mine");
    let refused = project::new_project(&settings, "-", "Add a contact form", project::Place::At(&mine, None), None, false, None);
    assert!(refused.is_err(), "a folder with files needs to know what they are");
    let ws = project::new_project(&settings, "-", "Add a contact form", project::Place::At(&mine, Some(ExistingFiles::Edit)), None, false, None).unwrap();
    let b = brief(&ws, "website", false, false, None, false, None);
    assert!(b.contains("already has their files (index.html)") && b.contains("use the workspace folder itself (./)") && b.contains("craft.json"), "{b}");

    let start = folder("start");
    let ws = project::new_project(&settings, "-", "A site like this", project::Place::At(&start, Some(ExistingFiles::StartFrom)), None, false, None).unwrap();
    let b = brief(&ws, "website", false, false, None, false, None);
    assert!(b.contains("which already had these files: index.html") && b.contains("bring into app/"), "{b}");
    assert!(!b.contains("use the workspace folder itself"));
}

#[test]
fn a_project_started_in_a_picked_folder_stays_in_the_list() {
    let mut session = craft_anything_engine::testing::session();
    let d = tempfile::tempdir().unwrap();
    let ws = project::new_project(&session.settings, "-", "A notes app", project::Place::At(&d.path().join("mine"), None), None, false, None).unwrap();
    session.remember_project(&ws.root).unwrap();
    assert_eq!(session.settings.project_folders, std::slice::from_ref(&ws.root));
    let saved = craft_anything_engine::Settings::load_from(&session.config_dir().join("config.toml"));
    assert_eq!(saved.project_folders, std::slice::from_ref(&ws.root), "saved in the session's own settings");
    let listed = Workspace::list_in(&session.settings.project_locations());
    assert!(listed.iter().any(|w| w.root == ws.root));
}

#[test]
fn starting_a_project_at_existing_project_folder_attaches_to_it() {
    let session = craft_anything_engine::testing::session();
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("mine");
    let ws = project::new_project(&session.settings, "-", "Initial app", project::Place::At(&folder, None), None, false, None).unwrap();
    assert_eq!(ws.root, folder);
    // Starting again at the same folder attaches to the existing project rather than failing or duplicating
    let reattached = project::new_project(&session.settings, "-", "Follow up", project::Place::At(&folder, None), None, false, None).unwrap();
    assert_eq!(reattached.root, folder);
    assert_eq!(reattached.manifest.name, ws.manifest.name);
}

/// #987 made a folder that already holds a project attach to it. Attaching must not treat the
/// project as new: its saved builder, asset budget, generate_assets and stack stay, and the typed
/// request becomes a follow-up instead of being dropped.
#[test]
fn reattaching_keeps_the_projects_saved_settings() {
    let session = craft_anything_engine::testing::session();
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("mine");
    let mut ws = project::new_project(&session.settings, "-", "Initial app", project::Place::At(&folder, None), None, false, Some("Claude")).unwrap();
    ws.set_person(|m| {
        m.asset_budget_usd = 42.0;
        m.generate_assets = true;
        m.builder = Some("Claude".into());
    })
    .unwrap();
    ws.manifest.stack = "Godot".into();
    ws.save().unwrap();

    let reattached = project::new_project(&session.settings, "-", "Add a jump", project::Place::At(&folder, None), Some("unity"), false, None).unwrap();
    assert_eq!(reattached.manifest.builder.as_deref(), Some("Claude"), "the saved builder is kept");
    assert_eq!(reattached.manifest.asset_budget_usd, 42.0, "the saved asset budget is kept");
    assert!(reattached.manifest.generate_assets, "the saved asset setting is kept");
    assert_eq!(reattached.manifest.stack, "Godot", "the saved stack is kept");
    assert_eq!(reattached.manifest.request, "Initial app", "the project's original request stays");
    assert!(project::has_project(&folder), "the folder is now a project");
    assert_eq!(
        project::attached_request(project::has_project(&folder), "Add a jump").as_deref(),
        Some("Add a jump"),
        "a request typed for an attached project is its follow-up"
    );
    assert_eq!(project::attached_request(false, "A new app"), None, "a new project's request stays its own");
}

/// A folder that doesn't hold a project yet is created with the stack, builder, asset budget and
/// generate_assets it was started with; only an attached folder keeps what it had.
#[test]
fn a_new_project_gets_the_settings_it_started_with() {
    let session = craft_anything_engine::testing::session();
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("new");
    let ws = project::new_project(&session.settings, "-", "A game", project::Place::At(&folder, None), Some("godot"), true, Some("Claude")).unwrap();
    assert_eq!(ws.manifest.stack, "Godot", "the picked stack is written");
    assert_eq!(ws.manifest.builder.as_deref(), Some("Claude"), "the picked builder is written");
    assert!(ws.manifest.generate_assets, "the asset choice is written");
    assert_eq!(ws.manifest.asset_budget_usd, session.settings.assets.budget(), "the budget comes from settings");
    assert_eq!(ws.manifest.request, "A game");
}

#[test]
fn a_project_edited_in_place_keeps_what_its_files_are_built_with() {
    use craft_anything_engine::workspace::ExistingFiles::{Edit, StartFrom};
    let first = |file: &str, request: &str, existing| {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(file), "").unwrap();
        let ws = Workspace::create_at(d.path(), "Mine", request, TargetRef::None, Some(existing)).unwrap();
        project::first_stack(&KindRegistry::built_in(), &ws, false)
    };
    assert_eq!(first("project.godot", "Fix my game's jump", Edit), Some("Godot"), "not the Unity its words would get");
    assert_eq!(first("requirements.txt", "Add a settings page", Edit), Some("Python"), "not the app kind's default");
    assert_eq!(first("requirements.txt", "Move it to Electron", Edit), Some("Electron"), "the person's words come first");
    assert_eq!(first("notes.md", "Fix my game's jump", Edit), Some("Unity"), "nothing to read: as for a new project");
    assert_eq!(first("project.godot", "Fix my game's jump", StartFrom), Some("Unity"), "material to start from doesn't decide");
}

/// A folder of the person's own with this file in it, as a project of `existing`.
fn own_folder(file: &str, existing: craft_anything_engine::workspace::ExistingFiles) -> (tempfile::TempDir, Workspace) {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join(file), "<p>hi</p>").unwrap();
    let ws = Workspace::create_at(d.path(), "Mine", "Add a contact form", TargetRef::None, Some(existing)).unwrap();
    (d, ws)
}

#[test]
fn a_persons_own_project_is_judged_by_its_own_build_and_tests_whatever_its_kind() {
    use craft_anything_engine::workspace::ExistingFiles::Edit;
    let (_d, ws) = own_folder("index.html", Edit);
    let r = check_worker(&ws, false).run_checks(&ws, &GameKind, 1).unwrap();
    assert_eq!(r.iter().map(|c| c.check.as_str()).collect::<Vec<_>>(), [CheckResult::BUILD, kinds::own::OwnTestsCheck::ID]);
    assert!(r.iter().all(|c| c.passed), "{r:?}");
    assert!(r[1].note.starts_with("No tests ran"), "says what wasn't run: {}", r[1].note);
}

/// A round of the builder's own checks has a time limit and looks at Pause before each check. A
/// check that couldn't start is a failed result with the reason, never a silent wait.
#[test]
fn a_self_check_round_stops_at_pause_and_at_its_time_limit() {
    use craft_anything_engine::workspace::ExistingFiles::Edit;
    let (_d, ws) = own_folder("index.html", Edit);
    let round = self_check::SELF_CHECK_ROUND;
    // Past its limit before the first check: that check is reported as not run, and nothing runs.
    let r = check_worker(&ws, false).run_checks_within(&ws, &GameKind, round, Some(Duration::ZERO)).unwrap();
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(!r[0].passed && r[0].note.starts_with("Not run:"), "{r:?}");
    // Paused: the round ends with the cancelled error before any check starts.
    let paused = check_worker(&ws, false);
    paused.stop.cancel();
    let e = paused.run_checks_within(&ws, &GameKind, round, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::Cancelled);
    // With time left, both checks run as they do without a limit.
    let r = check_worker(&ws, false).run_checks_within(&ws, &GameKind, round, Some(Duration::from_secs(600))).unwrap();
    assert_eq!(r.len(), 2, "{r:?}");
    assert!(r.iter().all(|c| c.passed), "{r:?}");
}

/// Each check is said before it runs, as a step of its own, so a stall names the check it is in.
/// A real round's lines are Check steps (the feed, the CLI and the progress panel keep their
/// stage); the builder's own checks belong to its Build turn.
#[test]
fn every_check_is_said_before_it_runs_as_its_own_step() {
    use craft_anything_engine::workspace::ExistingFiles::Edit;
    let (_d, ws) = own_folder("index.html", Edit);
    let run_round = |round: u32| {
        let mut worker = check_worker(&ws, false);
        let (tx, rx) = std::sync::mpsc::channel();
        worker.tx = tx;
        let r = worker.run_checks_within(&ws, &GameKind, round, None).unwrap();
        let said: Vec<(Step, String)> = rx
            .try_iter()
            .filter_map(|e| match e {
                RunEvent::Step { step, text } => Some((step, text)),
                _ => None,
            })
            .collect();
        (r, said)
    };
    let (r, said) = run_round(1);
    assert_eq!(r.len(), 2, "{r:?}");
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said.iter().all(|(s, _)| *s == Step::Check), "a real round stays in the Check stage: {said:?}");
    assert!(said[0].1.contains("Checking the build (1 of 2): build"), "{said:?}");
    assert!(said[1].1.contains("(2 of 2): own-tests"), "{said:?}");
    let (_, said) = run_round(self_check::SELF_CHECK_ROUND);
    assert!(said.iter().all(|(s, _)| *s == Step::Build), "the builder's own checks are part of its turn: {said:?}");
    assert!(said[0].1.starts_with("Checking its work (1 of 2): build"), "{said:?}");
}

#[test]
fn a_persons_own_project_is_asked_for_nothing_it_would_add_for_our_sake() {
    use craft_anything_engine::workspace::ExistingFiles::{Edit, StartFrom};
    let (_d, mut own) = own_folder("main.py", Edit);
    for k in ["app", "game"] {
        let b = brief(&own, k, false, false, None, false, None);
        assert!(!b.contains("--screenshot") && !b.contains("--self-test") && !b.contains("MECHANICS.md"), "{k}: {b}");
        assert!(b.contains("its own build and tests") && !b.contains(kind(k).goal()), "{k}: {b}");
    }
    let (_d, site) = own_folder("index.html", Edit);
    let b = brief(&site, "website", false, false, None, false, None);
    assert!(!b.contains(kinds::website::HOW_TO_RUN), "their own site keeps its own README: {b}");
    own.manifest.stack = "Unity".into();
    let b = brief(&own, "game", false, false, Some(Path::new("/x/Unity")), false, None);
    assert!(!b.contains("CraftBuild") && b.contains("keep to what the project already uses") && b.contains("/x/Unity"), "{b}");
    own.manifest.generate_assets = true;
    let b = brief(&own, "game", false, false, None, true, None);
    assert!(b.contains("Assets/Generated/") && !b.contains("--screenshot"), "what the person turned on still reaches it: {b}");
    let (_d, beside) = own_folder("main.py", StartFrom);
    let b = brief(&beside, "app", false, false, None, false, None);
    assert!(b.contains("--screenshot") && !b.contains("its own build and tests"), "a new app beside the files proves itself: {b}");
}

#[test]
fn a_site_stack_with_nothing_to_recreate_is_a_website() {
    let vite = "React + TypeScript (Vite)";
    assert_eq!(kind_of("Add a contact form", &TargetRef::None, "Plain HTML and CSS"), "website");
    assert_eq!(kind_of("A calculator in react", &TargetRef::None, vite), "website", "it can't be started as a program");
    assert_eq!(kind_of("Add a high score to my game", &TargetRef::None, vite), "game", "the person's words come first");
    assert_eq!(kind_of("Add a settings page", &TargetRef::None, "Python"), "app");
    assert_eq!(kind_of("Add a settings page", &TargetRef::None, "Electron"), "app", "a window, not a site");
    assert_eq!(kind_of("Copy this tool", &TargetRef::Local { path: "/x/tool".into() }, vite), "app", "an original decides");
}

#[test]
fn the_extractors_transcript_is_one_of_craft_anythings_own_files() {
    assert!(craft_anything_engine::workspace::OWN_NAMES.contains(&brief::EXTRACT_TRANSCRIPT), "a picked folder with this name would be overwritten");
}

#[test]
fn a_project_is_built_by_the_ai_it_was_started_with_while_that_one_is_on() {
    use crate::{AiConfig, BackendConfig, BackendEntry, BackendKind};
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    ws.manifest.builder = Some("Codex".into());
    let mut cfg = AiConfig::default();
    for (name, enabled) in [("Claude Code", true), ("Codex", true)] {
        let c = BackendConfig { name: name.into(), kind: BackendKind::AgentCli, model: String::new(), base_url: None, ssh_host: None, enabled };
        cfg.upsert(BackendEntry { config: c, options: Default::default() });
    }
    assert_eq!(super::project_builder(&cfg, &ws), Some("Codex"));
    assert_eq!(builder_backend(&cfg, super::project_builder(&cfg, &ws)).unwrap().config.name, "Codex");
    // Turned off (or removed): the default builds it instead of failing.
    cfg.backends.iter_mut().for_each(|b| b.config.enabled = b.config.name != "Codex");
    assert_eq!(super::project_builder(&cfg, &ws), None);
    assert_eq!(builder_backend(&cfg, super::project_builder(&cfg, &ws)).unwrap().config.name, "Claude Code");
}

/// The headless surfaces (CLI, MCP, a question) use the same replacement report as the app.
#[test]
fn a_saved_builder_that_is_off_or_gone_is_named_for_headless_starts() {
    use crate::{AiConfig, BackendConfig, BackendEntry, BackendKind};
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    ws.manifest.builder = Some("Codex".into());
    let mut cfg = AiConfig::default();
    for (name, enabled) in [("Claude Code", true), ("Codex", true)] {
        let c = BackendConfig { name: name.into(), kind: BackendKind::AgentCli, model: String::new(), base_url: None, ssh_host: None, enabled };
        cfg.upsert(BackendEntry { config: c, options: Default::default() });
    }
    assert_eq!(super::builder_replacement(&cfg, &ws, None), None, "an enabled saved builder is not a replacement");
    assert_eq!(super::builder_replacement(&cfg, &ws, Some("Codex")), None, "an explicit name is never a replacement");

    cfg.backends.iter_mut().for_each(|b| b.config.enabled = b.config.name != "Codex");
    let r = super::builder_replacement(&cfg, &ws, None).expect("a turned-off saved builder is replaced");
    assert_eq!((r.saved.as_str(), r.used.as_str()), ("Codex", "Claude Code"));
    let notice = super::replaced_builder_notice(&r.saved, &r.used);
    assert!(notice.contains("Codex") && notice.contains("Claude Code") && notice.contains("instead"), "{notice}");

    ws.manifest.builder = Some("Gone".into());
    let r = super::builder_replacement(&cfg, &ws, None).expect("a removed saved builder is replaced");
    assert_eq!((r.saved.as_str(), r.used.as_str()), ("Gone", "Claude Code"));
}

/// A question answered by the default AI still says which saved builder it replaced.
#[cfg(unix)]
#[test]
fn a_question_names_a_replaced_saved_builder() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let script = d.path().join("claude");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"In app/src.\"}]}}'\nprintf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1}'\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engine = std::sync::Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let hub = crate::AiHub::with_stores(engine, d.path().join("config.toml"), crate::KeyStore::file_only(d.path().join("keys.toml")));
    let mut settings = craft_anything_engine::testing::session().settings;
    settings.ai = toml::from_str(&format!(
        "[[backends]]\nname = \"Off\"\nkind = \"anthropic\"\nmodel = \"claude-test\"\nenabled = false\n\n[[backends]]\nname = \"Claude\"\nkind = \"agent_cli\"\nmodel = \"claude-code\"\ncommand = \"{}\"\nenabled = true\n",
        script.display()
    ))
    .unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    ws.set_person(|m| m.builder = Some("Off".into())).unwrap();
    let answered = super::question::ask(&hub, &settings, None, &ws, "where is it?", &CancelToken::default()).unwrap();
    assert_eq!(answered.backend, "Claude");
    let notice = answered.builder_notice.as_deref().unwrap_or_default();
    assert!(notice.contains("Off") && notice.contains("Claude"), "{notice}");
}

/// A name given on purpose is never ignored: a backend that is off is refused in words that say
/// how to fix it, and a name that matches nothing names the name instead of blaming the setup.
#[test]
fn a_named_backend_that_is_off_or_unknown_is_refused() {
    use crate::{AiConfig, BackendConfig, BackendEntry, BackendKind};
    let mut cfg = AiConfig::default();
    for (name, enabled) in [("Claude Code", true), ("Codex", false)] {
        let c = BackendConfig { name: name.into(), kind: BackendKind::AgentCli, model: String::new(), base_url: None, ssh_host: None, enabled };
        cfg.upsert(BackendEntry { config: c, options: Default::default() });
    }
    assert_eq!(builder_backend(&cfg, Some("Claude Code")).unwrap().config.name, "Claude Code");

    let off = builder_backend(&cfg, Some("Codex")).unwrap_err();
    assert_eq!(off.code, ErrorCode::BadSettings);
    assert!(off.message.contains("Codex") && off.message.contains("turned off"), "{off:?}");
    assert!(off.remediation.as_deref().is_some_and(|r| r.contains("Turn it on")), "{off:?}");

    let unknown = builder_backend(&cfg, Some("No Such AI")).unwrap_err();
    assert!(unknown.message.contains("No AI backend named"), "{unknown:?}");
    assert!(unknown.remediation.as_deref().is_some_and(|r| r.contains("ai list")), "{unknown:?}");
}

/// #446: a project copied from another computer keeps the id of a Claude Code session that lives
/// only there. The builder (a stand-in that answers like Claude Code 2.1) can't resume it, so the
/// turn starts a new session from the brief and what's in app/, and keeps the new id.
#[cfg(unix)]
#[test]
fn a_session_left_on_another_computer_starts_a_new_one_from_the_project() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    ws.manifest.agent_session = Some("39b5b312-5678-1234-961c-32112666ffab".into());
    ws.manifest.last_reply = "Built a notes app with a sidebar.".into();
    ws.save().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let program = bin.path().join("claude");
    let missing = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":0,"session_id":"39b5b312-5678-1234-961c-32112666ffab","errors":["No conversation found with session ID: 39b5b312-5678-1234-961c-32112666ffab"]}"#;
    let fresh = r#"{"type":"system","subtype":"init","session_id":"new-1"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Carried on: %s"}]}}
{"type":"result","subtype":"success","is_error":false,"num_turns":1,"session_id":"new-1"}
"#;
    // Like Claude Code: `--resume` of a session it doesn't have fails at once; otherwise it starts
    // one. It keeps what it was sent, so the test can read the new session's first message.
    let script = format!(
        "#!/bin/sh\ncase \" $* \" in *\" --resume \"*) cat >/dev/null; echo '{missing}'; echo 'No conversation found' >&2; exit 1;; esac\ncat > \"$0.last\"\nprintf '{fresh}' yes\n"
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config =
        crate::BackendConfig { name: "Claude".into(), kind: BackendKind::AgentCli, model: "claude-code".into(), base_url: None, ssh_host: None, enabled: true };
    let options = crate::config::BackendOptions { command: Some(program.display().to_string()), ..Default::default() };
    let spec = crate::cli_agents::CliSpec { resume: ws.manifest.agent_session.clone(), ..crate::cli_agents::CliSpec::new(config, options, false, None) };
    let mut agent = crate::cli_agents::CliConversation::new(spec);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let first = resumed_elsewhere("THE BRIEF", &ws.manifest.last_reply, None);
    let reply = w.turn(&mut ws, &mut agent, "The person asks: make it blue", true, Some(first)).unwrap();
    assert_eq!(reply.trim(), "Carried on: yes");
    let sent = std::fs::read_to_string(bin.path().join("claude.last")).unwrap();
    assert!(
        sent.starts_with("THE BRIEF") && sent.contains("already under way") && sent.contains("app/") && sent.contains("notes app with a sidebar"),
        "{sent}"
    );
    assert_eq!(ws.manifest.agent_session.as_deref(), Some("new-1"), "the new session is the one kept");
    assert_eq!(Workspace::open(&ws.root).unwrap().manifest.agent_session.as_deref(), Some("new-1"));
    let events: Vec<RunEvent> = rx.try_iter().collect();
    assert!(!events.iter().any(|e| matches!(e, RunEvent::Agent(AgentEvent::Error { .. }))), "nothing failed, as far as the person can see: {events:?}");
    assert!(
        events.iter().any(|e| matches!(e, RunEvent::Step { text, .. } if text.contains("isn't on this computer"))),
        "but they're told why it starts afresh"
    );
}

/// Without anything to start a new session from, the run stops and says why, and the id it
/// couldn't find is forgotten so the next run starts afresh.
#[cfg(unix)]
#[test]
fn a_missing_session_with_nothing_to_start_from_is_said_plainly() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    ws.manifest.agent_session = Some("gone".into());
    let bin = tempfile::tempdir().unwrap();
    let program = bin.path().join("claude");
    let missing = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":0,"session_id":"gone","errors":["No conversation found with session ID: gone"]}"#;
    std::fs::write(&program, format!("#!/bin/sh\ncat >/dev/null\necho '{missing}'\nexit 1\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config =
        crate::BackendConfig { name: "Claude".into(), kind: BackendKind::AgentCli, model: "claude-code".into(), base_url: None, ssh_host: None, enabled: true };
    let options = crate::config::BackendOptions { command: Some(program.display().to_string()), ..Default::default() };
    let spec = crate::cli_agents::CliSpec { resume: Some("gone".into()), ..crate::cli_agents::CliSpec::new(config, options, false, None) };
    let mut agent = crate::cli_agents::CliConversation::new(spec);
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let e = w
        .turn_with_wait(&mut ws, &mut agent, "Carry on.", true, None, |_, _| panic!("a missing session without a fresh start must not be retried"))
        .unwrap_err();
    assert!(e.message.contains("isn't on this computer"), "{e:?}");
    assert!(ws.manifest.agent_session.is_none());
}

/// #451: after each turn the project keeps a copy of the builder's session, and a new session on
/// another computer is handed the conversation it kept.
#[test]
fn the_builders_conversation_travels_with_the_project() {
    struct KeepsSessions(PathBuf);
    impl crate::cli_agents::AgentAdapter for KeepsSessions {
        fn ids(&self) -> &'static [&'static str] {
            &["keeps"]
        }
        fn args(&self, _cx: &crate::cli_agents::ArgsCx) -> Vec<String> {
            Vec::new()
        }
        /// Sessions sit flat in the stand-in store, named by id.
        fn session_file(&self, id: &str) -> Option<PathBuf> {
            Some(self.0.join(format!("{id}.jsonl"))).filter(|p| p.is_file())
        }
        /// Read back as Claude Code's are.
        fn handoff(&self, saved: &str, max: usize) -> Option<String> {
            crate::cli_agents::adapters::claude_handoff(saved, max)
        }
        /// Put back into the stand-in store, never over one that's there.
        fn restore_session(&self, id: &str, saved: &[u8], _cwd: &Path) -> Result<bool, ToolError> {
            let path = self.0.join(format!("{id}.jsonl"));
            if path.is_file() {
                return Ok(false);
            }
            std::fs::write(path, saved).map(|_| true).map_err(|e| ToolError::invalid(e.to_string()))
        }
    }
    let d = tempfile::tempdir().unwrap();
    let mut ws = Workspace::create(d.path(), "Notes", "A notes app", TargetRef::None).unwrap();
    let store = tempfile::tempdir().unwrap();
    let session = [
        serde_json::json!({"type":"user","message":{"content":"Make a notes app."}}),
        serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"Built it with a sidebar."}]}}),
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join("\n");
    std::fs::write(store.path().join("s-1.jsonl"), &session).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(craft_anything_engine::testing::session())),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };
    let adapter = KeepsSessions(store.path().to_path_buf());
    // No session yet, or one this computer doesn't have: nothing to keep, nothing said.
    w.keep_session(&ws, &adapter);
    ws.manifest.agent_session = Some("gone".into());
    w.keep_session(&ws, &adapter);
    assert!(!ws.agent_session_path().exists() && rx.try_iter().next().is_none());
    ws.manifest.agent_session = Some("s-1".into());
    w.keep_session(&ws, &adapter);
    assert_eq!(std::fs::read_to_string(ws.agent_session_path()).unwrap(), session);

    // On the other computer: the session is put back from the project, and the person is told.
    std::fs::remove_file(store.path().join("s-1.jsonl")).unwrap();
    w.restore_session(&ws, &adapter);
    assert_eq!(std::fs::read_to_string(store.path().join("s-1.jsonl")).unwrap(), session);
    let said: Vec<String> = rx.try_iter().filter_map(|e| if let RunEvent::Agent(AgentEvent::Status { text }) = e { Some(text) } else { None }).collect();
    assert!(said.iter().any(|t| t.contains("Brought the builder's earlier conversation over")), "{said:?}");
    w.restore_session(&ws, &adapter);
    assert!(rx.try_iter().next().is_none(), "nothing to say when it's already there");

    // Where it can't be, the new session's first message carries the kept conversation.
    let handoff = crate::cli_agents::AgentAdapter::handoff(&adapter, &kept_session(&ws).unwrap(), HANDOFF_CHARS);
    let first = resumed_elsewhere("THE BRIEF", "Last summary.", handoff.as_deref());
    assert!(first.starts_with("THE BRIEF") && first.contains("isn't on this computer"), "{first}");
    assert!(first.contains("Craft Anything asked you: Make a notes app.") && first.contains("You answered: Built it with a sidebar."), "{first}");
    // Without a kept conversation, the last summary stands in for it.
    let bare = resumed_elsewhere("THE BRIEF", "Last summary.", None);
    assert!(bare.contains("Last summary.") && !bare.contains("as the project kept it"), "{bare}");

    // A copy that can't be written is said, and the build goes on.
    std::fs::remove_file(ws.agent_session_path()).unwrap();
    std::fs::create_dir_all(ws.agent_session_path()).unwrap();
    w.keep_session(&ws, &adapter);
    let said: Vec<String> = rx.try_iter().filter_map(|e| if let RunEvent::Agent(AgentEvent::Status { text }) = e { Some(text) } else { None }).collect();
    assert!(said.iter().any(|t| t.contains("Couldn't keep a copy")), "{said:?}");
}

/// A coding agent that was signed in once and no longer is says so, rather than "isn't signed in",
/// which reads as if the person never did.
#[test]
fn a_sign_in_that_stopped_working_is_called_that() {
    assert_eq!(not_signed_in("claude-code", false), "Claude Code isn't signed in on this computer.");
    assert_eq!(not_signed_in("claude-code", true), "Claude Code's sign-in on this computer has expired or was signed out.");
}

#[test]
fn a_build_says_when_it_waits_for_an_analysis_started_earlier() {
    let d = tempfile::Builder::new().prefix("ca").tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Game", "Copy this game", TargetRef::None).unwrap();
    let launcher = d.path().join("analyzeHeadless");
    std::fs::write(&launcher, "").unwrap();
    let program = d.path().join("game.bin");
    std::fs::write(&program, b"not really a program").unwrap();
    let store = d.path().join("store");

    let mut s = craft_anything_engine::testing::session();
    s.settings.deep_mode = true;
    s.settings.engines.store_dir = Some(store.clone());
    s.settings.engines.ghidra.install_dir = Some(launcher);
    s.settings.engines.ghidra.startup_timeout_secs = Some(3);
    let (tx, rx) = std::sync::mpsc::channel();
    let w = Worker {
        wake: Default::default(),
        engine: Arc::new(Engine::new(s)),
        tx,
        stop: CancelToken::default(),
        preview: Arc::default(),
        seen: HashSet::new(),
        inbox: Inbox::new(ws.inbox_dir()),
        editor: None,
        notes: Arc::default(),
        feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
        keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
        kept_checks: Mutex::default(),
        assets: false,
        usage: None,
        self_check: None,
        reset_progress_after_notes: false,
    };

    // Another process is already analysing this program: a fresh lock beside its project.
    let sha = craft_anything_evidence::sha256_hex(&std::fs::read(&program).unwrap());
    let digest = craft_anything_evidence::sha256_hex(b"craft-anything-ghidra-fast-profile-v2|lang=");
    let project = craft_anything_engines::ghidra::project_cache_dir(&store, &sha, &digest);
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(project.with_extension("analysing"), "").unwrap();

    let done = w.prepare_ghidra(&program).expect("deep mode with Ghidra prepares the analysis");
    let _ = done.recv_timeout(Duration::from_secs(30));
    let said: Vec<String> = rx
        .try_iter()
        .filter_map(|e| match e {
            RunEvent::Step { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(said.iter().any(|t| t.contains("waiting for Ghidra's analysis of this program, started earlier")), "{said:?}");
}

/// A later run skips the study; in Deep mode it opens the program again, so the builder's tool
/// server has it and its first `native_*` call doesn't say "Nothing is open yet" (#1421).
#[test]
fn a_later_run_opens_the_projects_program_again_for_the_builder() {
    let d = tempfile::tempdir().unwrap();
    let program = d.path().join("game.bin");
    std::fs::write(&program, b"not really a program").unwrap();
    let target = TargetRef::Local { path: program.clone() };
    let app = kind("app");
    assert_eq!(study::program_to_reopen(&target, app.as_ref(), true), Some(program.clone()));
    assert_eq!(study::program_to_reopen(&target, app.as_ref(), false), None, "Clean room never opens the original");
    assert_eq!(study::program_to_reopen(&TargetRef::Local { path: d.path().join("gone.bin") }, app.as_ref(), true), None, "nothing to open");
    assert_eq!(study::program_to_reopen(&TargetRef::None, app.as_ref(), true), None);
    // Only a coding agent over SSH has its tools on the other computer; an API builder over SSH
    // (a tunnelled model) calls them here, so its program is reopened like a local one.
    assert!(study::tools_run_remotely(true, false), "coding agent over SSH");
    assert!(!study::tools_run_remotely(true, true), "API builder through an SSH tunnel");
    assert!(!study::tools_run_remotely(false, false) && !study::tools_run_remotely(false, true));

    for deep in [true, false] {
        let mut ws = Workspace::create(d.path(), &format!("Copy {deep}"), "Copy this app", target.clone()).unwrap();
        let mut s = craft_anything_engine::testing::session();
        s.settings.deep_mode = deep;
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut w = Worker {
            wake: Default::default(),
            engine: Arc::new(Engine::new(s)),
            tx,
            stop: CancelToken::default(),
            preview: Arc::default(),
            seen: HashSet::new(),
            inbox: Inbox::new(ws.inbox_dir()),
            editor: None,
            notes: Arc::default(),
            feed: Arc::new(Mutex::new(feed::FeedLog::open(&ws))),
            keys: crate::KeyStore::file_only(ws.root.join("keys.toml")),
            kept_checks: Mutex::default(),
            assets: false,
            usage: None,
            self_check: None,
            reset_progress_after_notes: false,
        };
        w.reopen_target(&mut ws, app.as_ref(), true, deep).unwrap();
        assert!(w.engine.session().shared_targets().is_empty(), "a builder over SSH isn't handed this computer's targets");
        w.reopen_target(&mut ws, app.as_ref(), false, deep).unwrap();
        let opened: Vec<PathBuf> = w.engine.session().shared_targets().into_iter().map(|t| t.path).collect();
        if deep {
            assert_eq!(opened, std::slice::from_ref(&program), "the builder's tool server is handed the program");
            w.reopen_target(&mut ws, app.as_ref(), false, deep).unwrap();
            assert_eq!(w.engine.session().shared_targets().len(), 1, "an open program isn't opened twice");
        } else {
            assert!(opened.is_empty(), "Clean room opens nothing");
        }
    }
}

/// #1178: both website briefs say the capture is the signed-in view only when the study's
/// capture.json says it was signed in.
#[test]
fn the_brief_says_signed_in_only_when_the_capture_was() {
    let d = tempfile::tempdir().unwrap();
    let ws = Workspace::create(d.path(), "Mail", "Clone it", TargetRef::Website { url: "https://mail.example.com/".into() }).unwrap();
    std::fs::create_dir_all(ws.capture_dir()).unwrap();
    for deep in [false, true] {
        assert!(!brief(&ws, "website", deep, false, None, false, None).contains(super::brief::SIGNED_IN_NOTE), "nothing captured yet");
    }
    let capture = ws.capture_dir().join("capture.json");
    std::fs::write(&capture, br#"{"signed_in": false}"#).unwrap();
    for deep in [false, true] {
        assert!(!brief(&ws, "website", deep, false, None, false, None).contains(super::brief::SIGNED_IN_NOTE));
    }
    std::fs::write(&capture, br#"{"signed_in": true, "signed_in_site": "mail.example.com"}"#).unwrap();
    for deep in [false, true] {
        assert!(brief(&ws, "website", deep, false, None, false, None).contains(super::brief::SIGNED_IN_NOTE), "deep: {deep}");
    }
    // A saved sign-in the site had ended: the page still asks for a password.
    std::fs::write(&capture, br#"{"signed_in": true, "sign_in_page": true}"#).unwrap();
    for deep in [false, true] {
        assert!(!brief(&ws, "website", deep, false, None, false, None).contains(super::brief::SIGNED_IN_NOTE), "deep: {deep}");
    }
    std::fs::write(&capture, b"garbage").unwrap();
    assert!(!brief(&ws, "website", false, false, None, false, None).contains(super::brief::SIGNED_IN_NOTE));
}
