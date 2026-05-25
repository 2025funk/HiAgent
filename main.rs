use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use eframe::egui;
use git2::{Repository, StatusOptions, Status};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([750.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HiAgent 智能体",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "simfang".to_string(),
                Arc::new(egui::FontData::from_static(include_bytes!("C:/Windows/Fonts/simfang.ttf"))),
            );
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                f.insert(0, "simfang".to_string());
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AgentApp::new()))
        }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileGitStatus {
    Untracked,
    Modified,
    Added,
}

impl FileGitStatus {
    fn to_str(&self) -> &'static str {
        match self {
            FileGitStatus::Untracked => "??",
            FileGitStatus::Modified => "M",
            FileGitStatus::Added => "A",
        }
    }

    fn color(&self) -> egui::Color32 {
        match self {
            FileGitStatus::Untracked => egui::Color32::WHITE,
            FileGitStatus::Modified => egui::Color32::YELLOW,
            FileGitStatus::Added => egui::Color32::GREEN,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitFileItem {
    path: String,
    status: FileGitStatus,
    staged: bool,
}

struct AgentApp {
    input: String,
    output: String,
    git_repo_path: Option<String>,
    git_files: Vec<GitFileItem>,
}

impl AgentApp {
    fn new() -> Self {
        Self {
            input: String::new(),
            output: "等待指令...".to_string(),
            git_repo_path: None,
            git_files: Vec::new(),
        }
    }

    fn refresh_git_status(&mut self, dir: &str) {
        self.git_repo_path = Some(dir.to_string());
        self.git_files.clear();

        let repo = match Repository::open(dir) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        let statuses = match repo.statuses(Some(&mut opts)) {
            Ok(s) => s,
            Err(_) => return,
        };

        for entry in statuses.iter() {
            let path = match entry.path() {
                Some(p) => p.to_string(),
                None => continue,
            };

            if path == ".gitignore" {
                continue;
            }

            let status = entry.status();
            let git_status;

            if status.contains(Status::INDEX_NEW) {
                git_status = FileGitStatus::Added;
            } else if status.contains(Status::WT_MODIFIED) || status.contains(Status::INDEX_MODIFIED) {
                git_status = FileGitStatus::Modified;
            } else {
                git_status = FileGitStatus::Untracked;
            }

            let staged = status.contains(Status::INDEX_NEW)
                || status.contains(Status::INDEX_MODIFIED);

            self.git_files.push(GitFileItem {
                path,
                status: git_status,
                staged,
            });
        }
    }

    fn run_command(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        self.output = format!("执行：{}\n\n", cmd);
        let res = self.execute(&cmd);
        self.output.push_str(&res);
        self.input.clear();
    }

    // 先刷新状态，再提交
    fn auto_local_git_backup(&mut self, file_path: &str) -> String {
        let file = Path::new(file_path);
        let dir = file.parent().unwrap_or(Path::new(".")).to_str().unwrap();
        let file_name = file.file_name().unwrap().to_str().unwrap();

        if !Path::new(dir).join(".git").exists() {
            let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git init", dir)).output();
        }

        let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git add {}", dir, file_name)).output();
        self.refresh_git_status(dir);
        let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git commit -m \"自动备份：{}\"", dir, file_name)).output();

        "\n✅ 已自动备份到本地Git版本".to_string()
    }

    fn git_log(&self, path: &str) -> String {
        let dir = Path::new(path).parent().unwrap_or(Path::new(".")).to_str().unwrap();
        let result = Command::new("cmd")
            .arg("/C")
            .arg(format!("cd /d {} && git log --oneline", dir))
            .output();

        match result {
            Ok(o) => format!("📜 Git历史记录：\n{}", String::from_utf8_lossy(&o.stdout)),
            Err(e) => format!("❌ 获取日志失败：{}", e),
        }
    }

    fn git_rollback(&mut self, path: &str) -> String {
        let dir = Path::new(path).parent().unwrap_or(Path::new(".")).to_str().unwrap();
        let result = Command::new("cmd")
            .arg("/C")
            .arg(format!("cd /d {} && git reset --hard HEAD^", dir))
            .output();

        self.refresh_git_status(dir);

        match result {
            Ok(o) => format!("⏪ 已回滚到上一版本：\n{}", String::from_utf8_lossy(&o.stdout)),
            Err(e) => format!("❌ 回滚失败：{}", e),
        }
    }

    fn rust_fmt(&self, file_path: &str) -> String {
        let file = Path::new(file_path);
        let dir = file.parent().unwrap_or(Path::new(".")).to_str().unwrap();
        let name = file.file_name().unwrap().to_str().unwrap();

        let result = Command::new("cmd")
            .arg("/C")
            .arg(format!("cd /d {} && rustfmt {}", dir, name))
            .output();

        match result {
            Ok(_) => "✅ 代码已自动格式化！".to_string(),
            Err(e) => format!("❌ 格式化失败：{}", e),
        }
    }

    fn execute(&mut self, cmd: &str) -> String {
        if cmd.starts_with("创建目录") {
            let p = cmd.replace("创建目录", "").trim().to_string();
            match fs::create_dir_all(&p) {
                Ok(_) => format!("✅ 目录已创建：{}", p),
                Err(e) => format!("❌ 失败：{}", e),
            }
        }
        else if cmd.starts_with("新建项目") {
            let p = cmd.replace("新建项目", "").trim().to_string();
            let result = Command::new("cmd")
                .arg("/C")
                .arg(format!("cargo new {}", p))
                .output();

            match result {
                Ok(o) => {
                    if !o.status.success() {
                        let err = String::from_utf8_lossy(&o.stderr);
                        return format!("❌ 新建项目失败：\n{}", err);
                    }

                    let out = String::from_utf8_lossy(&o.stdout);
                    let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git init", p)).output();
                    let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git add .", p)).output();
                    
                    self.refresh_git_status(&p);
                    
                    let _ = Command::new("cmd").arg("/C").arg(format!("cd /d {} && git commit -m \"project initialized\"", p)).output();

                    format!("✅ 新建项目完成 + Git 初始化成功：\n{}", out)
                }
                Err(e) => format!("❌ 新建项目失败：{}", e),
            }
        }
        else if cmd.starts_with("创建文件") {
            let p = cmd.replace("创建文件", "").trim().to_string();
            match fs::write(&p, "") {
                Ok(_) => {
                    let path = Path::new(&p).parent().unwrap_or(Path::new(".")).to_str().unwrap();
                    self.refresh_git_status(path);
                    format!("✅ 文件已创建：{}", p)
                }
                Err(e) => format!("❌ 失败：{}", e),
            }
        }
        else if cmd.starts_with("写入文件") {
            if !cmd.contains("内容是") {
                return "⚠️ 格式：写入文件 路径 内容是 内容".to_string();
            }
            let v: Vec<&str> = cmd.splitn(2, "内容是").collect();
            let f = v[0].replace("写入文件", "").trim().to_string();
            let c = v[1].trim().to_string();
            match fs::write(&f, &c) {
                Ok(_) => {
                    let mut s = format!("✅ 写入成功：{}", f);
                    s.push_str(&self.auto_local_git_backup(&f));
                    s
                }
                Err(e) => format!("❌ 失败：{}", e),
            }
        }
        else if cmd.starts_with("修改文件") {
            if !cmd.contains("把") || !cmd.contains("改成") {
                return "⚠️ 格式：修改文件 路径 把 旧 改成 新".to_string();
            }
            let s = cmd.replace("修改文件", "").trim().to_string();
            let v: Vec<&str> = s.splitn(2, "改成").collect();
            let left = v[0].trim();
            let new_c = v[1].trim();
            let v2: Vec<&str> = left.splitn(2, "把").collect();
            let f = v2[0].trim().to_string();
            let old_c = v2[1].trim().to_string();

            match fs::read_to_string(&f) {
                Ok(content) => {
                    let updated = content.replace(&old_c, new_c);
                    match fs::write(&f, &updated) {
                        Ok(_) => {
                            let mut s = format!("✅ 修改成功：{}", f);
                            s.push_str(&self.auto_local_git_backup(&f));
                            s
                        }
                        Err(e) => format!("❌ 写入失败：{}", e),
                    }
                }
                Err(e) => format!("❌ 读取失败：{}", e),
            }
        }
        else if cmd.starts_with("查看文件") {
            let f = cmd.replace("查看文件", "").trim().to_string();
            match fs::read_to_string(&f) {
                Ok(content) => format!("📄 文件内容：\n{}", content),
                Err(e) => format!("❌ 失败：{}", e),
            }
        }
        else if cmd.starts_with("查看历史") {
            let p = cmd.replace("查看历史", "").trim().to_string();
            self.git_log(&p)
        }
        else if cmd.starts_with("回滚版本") {
            let p = cmd.replace("回滚版本", "").trim().to_string();
            self.git_rollback(&p)
        }
        else if cmd.starts_with("格式化代码") {
            let p = cmd.replace("格式化代码", "").trim().to_string();
            self.rust_fmt(&p)
        }
        else if cmd.starts_with("运行") {
            let path = cmd.replace("运行", "").trim().to_string();
            let fp = Path::new(&path);

            if fp.join("Cargo.toml").exists() {
                let out = Command::new("cmd").arg("/C").arg(format!("cd /d {} && cargo clean && cargo run", fp.display())).output();
                match out {
                    Ok(o) => {
                        let out = String::from_utf8_lossy(&o.stdout);
                        let err = String::from_utf8_lossy(&o.stderr);
                        format!("📤 输出：\n{}\n❌ 错误：\n{}", out, err)
                    },
                    Err(e) => format!("❌ 失败：{}", e),
                }
            } else if path.ends_with(".rs") {
                let out = Command::new("cmd").arg("/C").arg(format!(
                    "cd /d {} && rustc {} && {}",
                    fp.parent().unwrap().display(),
                    fp.file_name().unwrap().to_str().unwrap(),
                    fp.file_stem().unwrap().to_str().unwrap()
                )).output();
                match out {
                    Ok(o) => {
                        let out = String::from_utf8_lossy(&o.stdout);
                        let err = String::from_utf8_lossy(&o.stderr);
                        format!("📤 输出：\n{}\n❌ 错误：\n{}", out, err)
                    },
                    Err(e) => format!("❌ 失败：{}", e),
                }
            } else {
                "⚠️ 不支持的文件".to_string()
            }
        }
        else {
            r#"
支持指令：
创建目录 路径
新建项目 路径
创建文件 路径
写入文件 路径 内容是 内容
修改文件 路径 把 旧 改成 新
查看文件 路径
查看历史 路径
回滚版本 路径
格式化代码 路径
运行 路径
"#.to_string()
        }
    }
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("HiAgent 本地智能体");
            ui.separator();

            // 输入框：固定高度，绝对不爆
            ui.label("输入指令：");
            ui.add_sized(
                [ui.available_width(), 120.0],
                egui::TextEdit::multiline(&mut self.input),
            );

            if ui.button("✅ 执行指令").clicked() {
                self.run_command();
            }

            ui.separator();

            // 结果区域
            ui.label("执行结果：");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(&self.output);
            });

            // Git 状态
            if let Some(path) = &self.git_repo_path {
                ui.separator();
                ui.strong(format!("📦 Git 仓库：{}", path));

                egui::Grid::new("git_grid")
                    .num_columns(3)
                    .spacing([12.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("状态");
                        ui.strong("暂存");
                        ui.strong("文件");
                        ui.end_row();

                        for item in &self.git_files {
                            ui.colored_label(item.status.color(), item.status.to_str());
                            ui.label(if item.staged { "✅" } else { "⬜" });
                            ui.label(&item.path);
                            ui.end_row();
                        }
                    });
            }
        });
    }
}